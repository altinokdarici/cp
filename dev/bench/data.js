window.BENCHMARK_DATA = {
  "lastUpdate": 1772827863367,
  "repoUrl": "https://github.com/altinokdarici/cp",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "altinokd@microsoft.com",
            "name": "Altinok Darici",
            "username": "altinokdarici"
          },
          "committer": {
            "email": "altinokd@microsoft.com",
            "name": "Altinok Darici",
            "username": "altinokdarici"
          },
          "distinct": true,
          "id": "fb59bb570c8fa297d62922f05bc788cc387c90ff",
          "message": "feat: add benchmark tracking to CI with PR comparison",
          "timestamp": "2026-03-06T11:51:45-08:00",
          "tree_id": "fc7910e41ce819b90aa10399655b4e7f8695bd17",
          "url": "https://github.com/altinokdarici/cp/commit/fb59bb570c8fa297d62922f05bc788cc387c90ff"
        },
        "date": 1772826945953,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "small (20 modules, 2 entries)",
            "value": 1240152,
            "unit": "ns/iter"
          },
          {
            "name": "medium (100 modules, 5 entries)",
            "value": 5408553,
            "unit": "ns/iter"
          },
          {
            "name": "large (500 modules, 10 entries)",
            "value": 26348554,
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "altinokd@outlook.com",
            "name": "Altinok Darici",
            "username": "altinokdarici"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4422bfe31e0c089932096975f227ae743dc07693",
          "message": "fix: support circular dependencies in module graph (#1)",
          "timestamp": "2026-03-06T12:10:30-08:00",
          "tree_id": "cb8b3be5db7149018c0d175f8205667afac360e1",
          "url": "https://github.com/altinokdarici/cp/commit/4422bfe31e0c089932096975f227ae743dc07693"
        },
        "date": 1772827862631,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "small (20 modules, 2 entries)",
            "value": 1237445,
            "unit": "ns/iter"
          },
          {
            "name": "medium (100 modules, 5 entries)",
            "value": 5423423,
            "unit": "ns/iter"
          },
          {
            "name": "large (500 modules, 10 entries)",
            "value": 26297156,
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}