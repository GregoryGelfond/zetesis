edge(a,x). edge(a,y). edge(b,z).
allowed(x). allowed(y) :- trigger.
{trigger; p(a); p(b)}.
:- edge(X,Y), allowed(Y), p(X).
