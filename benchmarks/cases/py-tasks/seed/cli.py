import argparse
from store import load
from render import render_text
def main():
    p=argparse.ArgumentParser(); p.add_argument('file'); args=p.parse_args()
    print(render_text(load(args.file)))
if __name__=='__main__': main()
