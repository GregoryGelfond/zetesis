nested(f(1)).
nested(f(a)).
number(1..2).
out(X) :- nested(f(X)), number(X).
