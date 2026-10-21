import json
def resolve(defaults, file_values, environment, cli):
    result = dict(defaults)
    result.update(file_values)
    for key, value in environment.items():
        if value:
            result[key] = json.loads(value)
    for key, value in cli.items():
        if value:
            result[key] = value
    return result
