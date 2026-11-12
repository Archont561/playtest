# ok: pcc-p05-exception-chaining-python
try:
    work()
except ValueError as error:
    raise RuntimeError('invalid') from error

# ruleid: pcc-p05-exception-chaining-python
try:
    work()
except ValueError as error:
    raise RuntimeError('invalid')
