n(-2..2).
d(1..2).
product(X,Y) :- n(X), d(Y), X*Y = 2, X/Y >= 0.
quotient(X,Y) :- n(X), d(Y), X/Y = 1.
magnitude(X,Y) :- n(X), d(Y), |X| = 2, X/Y >= 0.
power(X,Y) :- n(X), d(Y), X**2 = 4, X/Y >= 0.
