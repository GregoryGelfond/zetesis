row(3,3;3,4;4,4;6,6;6,7;7,7;9,9;12,12).
divisor(3).
divisible(X) :- row(X,X), divisor(D), X\D=0.
