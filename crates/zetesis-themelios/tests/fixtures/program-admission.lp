d(1).
d(2).
{p(X)} :- d(X).
-p(X) :- d(X), not p(X).
q(X) :- p(X), X != 2.
:- q(X), -p(X).
