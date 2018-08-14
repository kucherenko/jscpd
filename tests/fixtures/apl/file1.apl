:Namespace each_tests

I←{⍬≡⍴⍵:⍵ ⋄ ⊃((⎕DR ⍵)323)⎕DR ⍵}¯5000+?100⍴10000
F←100÷⍨?100⍴10000
B←?10⍴2

each∆dpii_TEST←'each∆R1'#.util.MK∆T2  I	I
each∆dpiis_TEST←'each∆R1'#.util.MK∆T2 4	5
each∆dpff_TEST←'each∆R1'#.util.MK∆T2  F	F
each∆dpif_TEST←'each∆R1'#.util.MK∆T2  I	F
each∆dpfi_TEST←'each∆R1'#.util.MK∆T2  F	I
each∆duffs_TEST←'each∆R2'#.util.MK∆T2 5.5	3.1
each∆duii_TEST←'each∆R2'#.util.MK∆T2  I	I
each∆duff_TEST←'each∆R2'#.util.MK∆T2  F	F
each∆duif_TEST←'each∆R2'#.util.MK∆T2  I	F
each∆dufi_TEST←'each∆R2'#.util.MK∆T2  F	I
each∆mui_TEST←'each∆R3'#.util.MK∆T2   I	(I~0)
each∆muf_TEST←'each∆R3'#.util.MK∆T2   F	F
each∆mub_TEST←'each∆R6'#.util.MK∆T2   B	B
each∆mpi_TEST←'each∆R4'#.util.MK∆T2   I	(I~0)
each∆mpf_TEST←'each∆R4'#.util.MK∆T2   F	F
each∆mpb_TEST←'each∆R5'#.util.MK∆T2   B	B
each∆durep_TEST←'each∆R7'#.util.MK∆T2 I (⍉⍪I)

:EndNamespace