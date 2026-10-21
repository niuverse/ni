def collect(fetch,start):
    items=[]; url=start
    while url is not None:
        page=fetch(url)
        if not page['items']: break
        items.extend(page['items'])
        url=page['next']
    return items
