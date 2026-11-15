% Unread atoms make the objective prefix sparse in the original atom universe.
noise(1..16).
1 { z_a; z_b; z_c } 1.
{ z_hidden }.
#minimize { 1,a:z_a; 1,b:z_b; 3,c:z_c }.
