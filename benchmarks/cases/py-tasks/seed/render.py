def render_text(tasks):
    return '\n'.join(f"{t['id']} [{t['status']}] {t['title']}" for t in tasks)
