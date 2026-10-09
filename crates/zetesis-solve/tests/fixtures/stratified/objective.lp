a.
b :- not c.
c :- d.
d :- c.
#minimize { 1@1,b : b }.
#show b/0.
