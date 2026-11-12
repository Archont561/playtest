# ok: pcc-p02-unsafe-dynamic-python
ast.literal_eval(text)

# ok: pcc-p02-unsafe-dynamic-python
yaml.safe_load(text)

# ruleid: pcc-p02-unsafe-dynamic-python
eval(text)

# ruleid: pcc-p02-unsafe-dynamic-python
yaml.load(text)
