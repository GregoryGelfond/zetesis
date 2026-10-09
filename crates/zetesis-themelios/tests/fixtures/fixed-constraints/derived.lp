edge(a,x). edge(a,y). edge(b,z) :- trigger.
allowed(x). allowed(y). allowed(z).
{trigger; p(a); p(b)}.
:- edge(X,Y), allowed(Y), p(X).
