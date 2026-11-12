# ok: pcc-p01-swallowed-exception-python
try:
    work()
except ValueError as error:
    raise RuntimeError('invalid') from error

# ruleid: pcc-p01-swallowed-exception-python
try:
    work()
except ValueError:
    pass

# ruleid: pcc-p01-swallowed-exception-python
try:
    work()
except:
    pass
