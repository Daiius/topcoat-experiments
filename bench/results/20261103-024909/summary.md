# Benchmark summary

- date: 2026-11-03T02:49:09+09:00
- commit: c24a249
- host: AMD Ryzen 7 8745H w/ Radeon 780M Graphics, 16 threads, 13 GiB, kernel 7.1.8+deb13-amd64
- docker 29.7.2, oha 1.16.0, curl 8.14.1
- limits: cpus 1, memory 2g (bench/compose.yml). oha: 20s x 32 / 200 rps


## Image size

| app | image | size |
|---|---|---|
| topcoat | `storefront-topcoat:bench` | 18.5 MiB |
| nextjs | `storefront-nextjs:bench` | 77.4 MiB |

## Cold start (docker start -> first 200 on /)

| app | runs | median | min | max | `docker start` | first `/products/1` TTFB |
|---|---|---|---|---|---|---|
| nextjs | 5 | 696.4 ms | 631.2 ms | 1355.4 ms | 181.8 ms | 7.7 ms |
| topcoat | 5 | 183.5 ms | 176.7 ms | 246.5 ms | 173.6 ms | 1.1 ms |

## Routes (median of 5, after one warm-up request)

| route | app | status | TTFB | total | identity | br | gzip | Cache-Control |
|---|---|---|---|---|---|---|---|---|
| `/` | nextjs | 200 | 4 ms | 12.6 ms | 161 KB | 161 KB (none) | 12.3 KB | private, no-cache, no-store, max-age=0, must-revalidate |
| `/` | topcoat | 200 | 1.3 ms | 1.4 ms | 39.2 KB | 2.2 KB | 2.5 KB | - |
| `/cart` | nextjs | 200 | 3 ms | 8.9 ms | 25.4 KB | 25.4 KB (none) | 6.9 KB | private, no-cache, no-store, max-age=0, must-revalidate |
| `/cart` | topcoat | 200 | 1.3 ms | 1.3 ms | 3.1 KB | 0.9 KB | 1 KB | - |
| `/products` | nextjs | 200 | 3 ms | 17.5 ms | 329.3 KB | 329.3 KB (none) | 18.5 KB | private, no-cache, no-store, max-age=0, must-revalidate |
| `/products` | topcoat | 200 | 1.4 ms | 1.5 ms | 80.8 KB | 3.8 KB | 4.4 KB | - |
| `/products/1` | nextjs | 200 | 3.2 ms | 209.1 ms | 150.5 KB | 150.5 KB (none) | 16.3 KB | private, no-cache, no-store, max-age=0, must-revalidate |
| `/products/1` | topcoat | 200 | 1.3 ms | 802 ms | 46.8 KB | 3.7 KB | 4.1 KB | - |
| `/products/9999` | nextjs | 404 | 3 ms | 8.5 ms | 26.7 KB | 26.7 KB (none) | 7 KB | private, no-cache, no-store, max-age=0, must-revalidate |
| `/products/9999` | topcoat | 404 | 1.1 ms | 1.1 ms | 0 KB | 0 KB (none) | 0 KB (none) | - |
| `/products?q=headphones` | nextjs | 200 | 2.9 ms | 12.1 ms | 208.8 KB | 208.8 KB (none) | 14.3 KB | private, no-cache, no-store, max-age=0, must-revalidate |
| `/products?q=headphones` | topcoat | 200 | 1.5 ms | 1.5 ms | 49.3 KB | 2.9 KB | 3.3 KB | - |

## Streaming of `/products/1` (median; bursts = data separated by > 50 ms, run 1)

| app | Accept-Encoding | first data | last data | bytes | bursts (start ms: bytes) |
|---|---|---|---|---|---|
| nextjs | br, gzip | 3.7 ms | 211.3 ms | 16780 | 5.2: 13532, 210.7: 3244 |
| nextjs | identity | 4.4 ms | 210.4 ms | 154223 | 4: 103069, 209.5: 51154 |
| topcoat | br, gzip | 1.7 ms | 802.8 ms | 3823 | 1.9: 2388, 203.4: 1025, 802.5: 410 |
| topcoat | identity | 1.4 ms | 801.9 ms | 47995 | 1.1: 11883, 201.8: 23721, 801.6: 12391 |

## Subresources referenced by the HTML (raw / br-or-gzip; images: viewport 1280px, DPR 1)

| page | app | JS | CSS | images (eager) | images (all) | non-200 |
|---|---|---|---|---|---|---|
| `/` | nextjs | 8 / 580.2 KB / 179.4 KB | 1 / 18.4 KB / 4.6 KB | 0 / 0 KB | 6 / 13.8 KB | 0 |
| `/` | topcoat | 1 / 47.4 KB / 15.3 KB | 1 / 17.3 KB / 4.2 KB | 0 / 0 KB | 6 / 1699.4 KB | 0 |
| `/cart` | nextjs | 8 / 580.2 KB / 179.4 KB | 1 / 18.4 KB / 4.6 KB | 0 / 0 KB | 0 / 0 KB | 0 |
| `/cart` | topcoat | 1 / 47.4 KB / 15.3 KB | 1 / 17.3 KB / 4.2 KB | 0 / 0 KB | 0 / 0 KB | 0 |
| `/products` | nextjs | 8 / 583.5 KB / 180.4 KB | 1 / 18.4 KB / 4.6 KB | 1 / 2.9 KB | 15 / 33.3 KB | 0 |
| `/products` | topcoat | 1 / 47.4 KB / 15.3 KB | 1 / 17.3 KB / 4.2 KB | 0 / 0 KB | 15 / 4229.9 KB | 0 |
| `/products/1` | nextjs | 8 / 580.8 KB / 179.6 KB | 1 / 18.4 KB / 4.6 KB | 1 / 5.4 KB | 4 / 12 KB | 0 |
| `/products/1` | topcoat | 1 / 47.4 KB / 15.3 KB | 1 / 17.3 KB / 4.2 KB | 1 / 290.5 KB | 3 / 844.5 KB | 0 |

## Load (oha)

| route | app | mode | RPS | p50 | p99 | success | statuses |
|---|---|---|---|---|---|---|---|
| `/` | nextjs | max | 94.6 | 294.5 ms | 788.7 ms | 100% | 200:1860 |
| `/` | topcoat | max | 5677 | 5.7 ms | 7.3 ms | 100% | 200:113538 |
| `/` | nextjs | rate | 98 | 6929.4 ms | 10312.5 ms | 100% | 200:1929 |
| `/` | topcoat | rate | 200 | 1 ms | 1.7 ms | 100% | 200:4000 |
| `/cart` | nextjs | max | 234.8 | 118.9 ms | 206.7 ms | 100% | 200:4665 |
| `/cart` | topcoat | max | 15095 | 2.1 ms | 2.9 ms | 100% | 200:301899 |
| `/cart` | nextjs | rate | 200 | 8.2 ms | 168.8 ms | 100% | 200:3999 |
| `/cart` | topcoat | rate | 200.1 | 0.8 ms | 1.4 ms | 100% | 200:4000 |
| `/products` | nextjs | max | 64.5 | 494.4 ms | 816.8 ms | 100% | 200:1258 |
| `/products` | topcoat | max | 2437.2 | 13.5 ms | 16.8 ms | 100% | 200:48717 |
| `/products` | nextjs | rate | 64.5 | 7262.5 ms | 13605.7 ms | 100% | 200:1258 |
| `/products` | topcoat | rate | 200 | 1.7 ms | 2.5 ms | 100% | 200:4000 |
| `/products/1` | nextjs | max | 90.2 | 359.3 ms | 555.4 ms | 100% | 200:1772 |
| `/products/1` | topcoat | max | 40 | 802.2 ms | 810.9 ms | 100% | 200:768 |
| `/products/1` | nextjs | rate | 96.2 | 5303.8 ms | 10389.2 ms | 100% | 200:1892 |
| `/products/1` | topcoat | rate | 40 | 8500 ms | 15566.3 ms | 100% | 200:768 |
| `/products/9999` | nextjs | max | 152.9 | 176.9 ms | 557.1 ms | 100% | 404:3026 |
| `/products/9999` | topcoat | max | 80393.2 | 0.4 ms | 0.5 ms | 100% | 404:1607991 |
| `/products/9999` | nextjs | rate | 194.1 | 467 ms | 741.9 ms | 100% | 404:3851 |
| `/products/9999` | topcoat | rate | 200 | 0.8 ms | 1.2 ms | 100% | 404:4000 |
| `/products?q=headphones` | nextjs | max | 79.4 | 404 ms | 667 ms | 100% | 200:1557 |
| `/products?q=headphones` | topcoat | max | 2739.9 | 8.7 ms | 42.3 ms | 100% | 200:54769 |
| `/products?q=headphones` | nextjs | rate | 79.7 | 6262.5 ms | 12065 ms | 100% | 200:1563 |
| `/products?q=headphones` | topcoat | rate | 200.1 | 1.2 ms | 1.6 ms | 100% | 200:4000 |

## Memory (docker stats)

| app | idle | after load |
|---|---|---|
| topcoat | 12.3 MiB | 26.9 MiB |
| nextjs | 92.6 MiB | 279.5 MiB |
