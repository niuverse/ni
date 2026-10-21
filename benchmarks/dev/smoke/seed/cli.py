import json,sys
from formatter import format_record
if __name__=='__main__': print(format_record(json.load(sys.stdin)))

