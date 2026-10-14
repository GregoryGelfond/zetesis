d(1..3).
blocked(2).
accepted(X) :- d(X), not blocked(X), X > 1.
-absent(1).
receipt(X) :- accepted(X).
:- receipt(1).
#show receipt/1.
