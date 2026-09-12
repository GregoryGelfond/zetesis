node(0).
node(X+1) :- node(X), X < 2.

left(X) | right(X) :- node(X).
:- left(0), left(1).

#minimize { 1@1,X : left(X), X < 2 }.
#show left/1.
