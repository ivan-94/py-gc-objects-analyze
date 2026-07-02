# Demo Transcript

This transcript shows the intended first-run experience after a GitHub Release and PyPI package are published. Replace `0.1.0` with the release you are testing.

```text
$ curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh
installed pygco to /home/alice/.local/bin/pygco
0.1.0

$ python -m pip install "pygco-dump[fastapi]"
Successfully installed pygco-dump-0.1.0 ...

$ pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser
pygco web: http://127.0.0.1:3791/
database: /home/alice/.cache/pygco/sessions/20260702T120000Z-abc12345/analysis.sqlite
```

Open the printed local URL and start on Overview:

![Overview screenshot](assets/web-ui/overview.png)

The fixture path is intentionally public test data. Do not create demo screenshots from private production dumps.
