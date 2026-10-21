import csv,io
from decimal import Decimal
def total(text):
    return str(sum((Decimal(row['amount']) for row in csv.DictReader(io.StringIO(text))),Decimal(0)))
