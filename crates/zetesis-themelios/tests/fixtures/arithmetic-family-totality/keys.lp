num(2).
divisor(1).
result(X,D) :- num(X), divisor(D), not blocked(D), X/D>0.
