digit(0..2).
sum(X+Y) :- digit(X), digit(Y).
:- digit(X), digit(Y), X+Y=2.
