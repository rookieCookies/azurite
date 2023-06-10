def fib_iter(n):
    a = 0.0
    b = 1.0
    i = 0.0
    while i < n:
        c = a + b 
        a = b
        b = c
        i = i + 1.0
    return a


fib_iter(10_000.0)
