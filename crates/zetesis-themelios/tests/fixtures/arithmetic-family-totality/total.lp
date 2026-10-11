num(2..6).
divisor(2..3).
result(X,D) :- divisor(D), num(X), D<X, X\D=0.
