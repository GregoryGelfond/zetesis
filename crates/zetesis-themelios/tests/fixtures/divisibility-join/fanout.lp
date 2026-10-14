row(3,1..10).
row(4,1..10).
divisor(3,known).
divisible(X) :- row(X,1), divisor(D,known), X\D=0.
