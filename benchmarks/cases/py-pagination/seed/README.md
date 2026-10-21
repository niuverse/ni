# Catalogue pagination
Repair collect(fetch,start). fetch returns {'items':[...], 'next':URL or None}.
Follow next through empty pages; stop only on None. Keep the first item for
each id in encounter order. Repeated URLs raise ValueError before refetching.
Fetch errors propagate. Do not mutate page data. URLs are opaque strings.
transport.py supplies a local HTTP JSON fetcher. No external service needed.
Existing tests: python -m unittest discover -s tests

Use only the standard library. Do not add dependencies or change build manifests.
