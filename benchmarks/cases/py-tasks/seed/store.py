import json
def load(path):
    with open(path,encoding='utf-8') as file: return json.load(file)
