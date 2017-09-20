#!/usr/bin/perl

use strict;
use warnings;

sub square {
    my ( $x ) = @_;

    return $x * $x;
}

print square($ARGV[0]) . "\n";

