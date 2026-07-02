# Python Producer 接入指南

本文面向需要把 `pygco_dump` 接入真实 Python 服务的开发者。核心原则是：被分析的 Python 进程只负责生成 gzip JSONL dump；触发方式由接入项目选择。

## 当前可用能力

`pygco_dump` 当前提供两层能力：

- `pygco_dump.write_gc_dump(file, ...)`：框架无关的底层 API，只需要一个 binary file-like object。
- `pygco_dump.fastapi.gc_heap_dump_route()`：FastAPI 便捷封装。

因此 HTTP endpoint 不是唯一入口。Celery worker、管理命令、信号处理、Unix domain socket、WSGI/ASGI handler 都可以在进程内调用 `write_gc_dump()`。

## 选择触发方式

| 场景 | 推荐方式 | 说明 |
| --- | --- | --- |
| FastAPI/HTTP 服务 | Debug endpoint | 最容易用 `curl` 拉取，适合已有 HTTP 控制面。 |
| 临时脚本或本地复现 | 直接写文件 | 适合开发环境、单进程服务、命令行管理任务。 |
| Celery `solo` 或只想采样某个 task 子进程 | Celery task | 简单，但不能覆盖所有 `prefork` 子进程。 |
| Celery/Gunicorn/uWSGI `prefork` | 按 PID 发信号 | 每个进程在自己内部 dump，适合多进程 fan-out。 |
| 需要传复杂参数 | Unix domain socket | 比信号更容易传 `collect`、输出路径、标签等参数。 |

默认建议：

- Web 服务先用 HTTP endpoint。
- Worker/daemon 先用信号触发。
- 多进程排查时保留每个 PID 的独立 dump，并用 collection manifest 记录同一轮采集关系。

## FastAPI endpoint

```python
from pygco_dump.fastapi import gc_heap_dump_route

app.add_api_route(
    "/debug/gc-heap-dump",
    gc_heap_dump_route(),
    methods=["GET"],
)
```

拉取：

```bash
curl -o before.jsonl.gz "http://service/debug/gc-heap-dump?collect=false"
curl -o after.jsonl.gz "http://service/debug/gc-heap-dump?collect=false"
```

endpoint 只导出 dump，不做聚合、不做分析。鉴权、内网暴露、路由开关由接入项目自己控制。

## 直接写文件

```python
from pathlib import Path

from pygco_dump import write_gc_dump


with Path("heap.jsonl.gz").open("wb") as file:
    write_gc_dump(file, collect=False, include_repr=False)
```

适合管理命令、一次性诊断脚本、开发环境复现。生成后用本地 CLI 分析：

```bash
pygco open heap.jsonl.gz
```

## Celery task 触发

Celery task 是最小接入方式：

```python
from pathlib import Path

from celery import shared_task
from pygco_dump import write_gc_dump


@shared_task(name="debug.gc_heap_dump")
def gc_heap_dump(path: str, collect: bool = False) -> dict[str, int]:
    with Path(path).open("wb") as file:
        summary = write_gc_dump(file, collect=collect, include_repr=False)
    return {
        "dumped_count": summary.dumped_count,
        "stub_count": summary.stub_count,
        "total_object_records": summary.total_object_records,
        "elapsed_ms": summary.elapsed_ms,
    }
```

限制：

- 默认 `prefork` 下，这个 task 只会在某一个 pool child 中执行。
- 它不能保证覆盖 worker master 进程或所有 child 进程。
- 如果需要高可信 before/after diff，尽量让两次 dump 来自同一个 PID，或在诊断 worker 中临时使用 `--concurrency=1`。

## 信号触发

信号适合 Celery、Gunicorn、uWSGI 等多进程模型，因为信号是按 PID 发送的。接入项目可以枚举目标进程，逐个发送 `SIGUSR2`，让每个进程写出自己的 dump 文件。

不要在 Python signal handler 里直接执行 dump。推荐 handler 只设置 `threading.Event`，由后台线程调用 `write_gc_dump()`。

下面的 `install_signal_gc_dump()` 是接入方应用代码示例，不是 `pygco_dump` 当前内置 API：

```python
from __future__ import annotations

import logging
import os
import signal
import socket
import threading
import time
import uuid
from pathlib import Path

from pygco_dump import DumpInProgressError, write_gc_dump

logger = logging.getLogger(__name__)
_dump_requested = threading.Event()
_install_lock = threading.Lock()
_installed = False


def install_signal_gc_dump(
    *,
    output_dir: str | os.PathLike[str] = "/tmp/pygco-dumps",
    signum: signal.Signals = signal.SIGUSR2,
    collect: bool = False,
) -> None:
    global _installed
    with _install_lock:
        if _installed:
            return
        Path(output_dir).mkdir(parents=True, exist_ok=True)
        signal.signal(signum, _request_dump)
        thread = threading.Thread(
            target=_dump_loop,
            kwargs={"output_dir": Path(output_dir), "collect": collect},
            name="pygco-signal-dump",
            daemon=True,
        )
        thread.start()
        _installed = True


def _request_dump(signum: int, frame: object) -> None:
    _ = signum, frame
    _dump_requested.set()


def _dump_loop(*, output_dir: Path, collect: bool) -> None:
    while True:
        _dump_requested.wait()
        _dump_requested.clear()
        path = output_dir / _dump_filename()
        try:
            with path.open("xb") as file:
                summary = write_gc_dump(file, collect=collect, include_repr=False)
            logger.warning("wrote pygco dump path=%s summary=%s", path, summary)
        except DumpInProgressError:
            logger.warning("pygco dump already running; skipped signal-triggered dump")
        except Exception:
            logger.exception("failed to write pygco dump path=%s", path)


def _dump_filename() -> str:
    timestamp = time.strftime("%Y%m%dT%H%M%S", time.gmtime())
    host = socket.gethostname()
    pid = os.getpid()
    suffix = uuid.uuid4().hex[:8]
    return f"heap.{host}.{pid}.{timestamp}.{suffix}.jsonl.gz"
```

采集时：

```bash
kill -USR2 <pid>
```

多进程采集时，接入方可以用自己的运维脚本枚举 master 和 child PID，然后逐个发送信号。输出目录中会得到多个 dump：

```text
/tmp/pygco-dumps/
  heap.worker-a.12001.20260702T120000.8ab13f01.jsonl.gz
  heap.worker-a.12002.20260702T120000.a31c920b.jsonl.gz
  heap.worker-a.12003.20260702T120000.7fd00129.jsonl.gz
```

分析：

```bash
pygco open /tmp/pygco-dumps/heap.*.jsonl.gz
```

注意：这些是同一轮多进程快照，不是同一进程连续快照。跨 PID 的 object id 生命周期 diff 可信度较低，优先看 type/module/cohort、reachable size、owner path 和 suspect leads。

## Celery `prefork` 接入信号

在 `prefork` 模式下，pool child 才是实际执行 task 的进程。需要在每个 child 初始化时安装信号入口：

```python
from celery.signals import worker_process_init

from myapp.debug_dump import install_signal_gc_dump


@worker_process_init.connect
def install_child_gc_dump(**kwargs: object) -> None:
    _ = kwargs
    install_signal_gc_dump(output_dir="/tmp/pygco-dumps")
```

如果怀疑 Celery worker master 进程本身泄漏，可以在 master 初始化路径单独安装一次，但要把 master dump 和 child dump 在 collection manifest 中区分清楚。

## Collection manifest

把多进程 dump 交给其他人分析时，建议同时保存一个 manifest。它不是 `pygco` 当前必需输入，但能避免下游误读证据：

```json
{
  "collection_id": "20260702T120000-worker-a",
  "created_at": "2026-07-02T12:00:00Z",
  "trigger": "SIGUSR2",
  "service": "orders-worker",
  "runtime": "celery-prefork",
  "dumps": [
    {
      "path": "heap.worker-a.12001.20260702T120000.8ab13f01.jsonl.gz",
      "pid": 12001,
      "role": "worker-child"
    }
  ],
  "notes": [
    "same collection round across multiple processes",
    "object lifecycle diff across pids is weak evidence"
  ]
}
```

## Safety checklist

- 默认 `collect=false`，避免诊断入口影响请求延迟。
- 默认 `include_repr=false`，避免执行用户自定义 `repr` 或输出敏感大字符串。
- 同一进程同一时间只允许一个 dump；遇到并发触发应跳过或返回冲突。
- 输出目录要在本机或容器内可写，并限制访问权限。
- 不要把 dump endpoint 或 signal 操作暴露给非受信用户。
- 记录 dump 开始、结束、文件路径、PID、耗时和对象数量。
- 在容器或 Kubernetes 中，确认 dump 目录会被保留或能被拷出。

Before enabling an HTTP endpoint outside a developer laptop, confirm:

- the route is behind an internal-only network boundary,
- the route is disabled by default or protected by an explicit runtime flag,
- operators know where dump files are written and how to delete them,
- `repr` output is disabled unless the investigation explicitly needs it,
- any shared dump has been reviewed for sensitive metadata.
