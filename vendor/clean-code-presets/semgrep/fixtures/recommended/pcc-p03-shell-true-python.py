# ok: pcc-p03-shell-true-python
subprocess.run(['git', 'status'], check=True)

# ruleid: pcc-p03-shell-true-python
subprocess.run('git status', shell=True)
