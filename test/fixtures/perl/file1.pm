package My::Package;

use strict;
use warnings;

=head1 square

Calculates the square of a number

=cut

sub square {
    my ( $x ) = @_;

    return $x * $x;
}

1;

