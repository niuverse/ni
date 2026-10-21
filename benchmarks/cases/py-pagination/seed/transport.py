import json
from urllib.request import urlopen
def fetch(url):
    with urlopen(url,timeout=5) as response: return json.load(response)
