1 { a; b } 1.
{ c; d; e; f; g; h }.
:- c,d,e.
:- c,d,f.
:- c,e,f.
:- d,e,f.
:- c,d,g.
:- c,e,g.
:- c,f,g.
:- d,e,g.
:- d,f,g.
:- e,f,g.
:- c,d,h.
:- c,e,h.
:- c,f,h.
:- c,g,h.
:- d,e,h.
:- d,f,h.
:- d,g,h.
:- e,f,h.
:- e,g,h.
:- f,g,h.
#minimize { 1,a:a; 2,b:b }.
