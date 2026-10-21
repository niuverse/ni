import json,sys
from client import collect
from transport import fetch
if __name__=='__main__': print(json.dumps(collect(fetch,sys.argv[1])))
