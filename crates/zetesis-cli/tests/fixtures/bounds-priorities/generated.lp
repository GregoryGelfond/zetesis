a. {b}.
n(N) :- N = #count {1:a; 1:b}.
1 {x;y} 1.
#minimize {2@N,x:n(N),x; 1@(N+1),y:n(N),y}.
