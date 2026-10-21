import json, sys
from config import resolve
from defaults import DEFAULTS
if __name__=='__main__':
    with open(sys.argv[1]) as file: print(json.dumps(resolve(DEFAULTS,json.load(file),{},{}),sort_keys=True))
