#!/usr/bin/perl


=head1

Author: Anthony Ettinger

License: GPL 2.0
URL: http://www.gnu.org/licenses/gpl.txt

Notes:

This script was originally written to function as a MySQL database backup
script in conjunction with the open source Perl/rsync backup program "rsnapshot".
rsnapshot can be found here: http://www.rsnapshot.org/

In order to backup a MySQL database remotely,
the necessary database user must be able
to connect remotely to the database server from
your IP number (some ISPs only allow access from localhost - you may need
to email your admin and ask for your ip to be given access)

It is extremely important that you secure the /etc/mysqlbackup file
so only YOU can read the file, 'chmod 0600 /etc/mysqlbackup',
as it will store the database passwords in plain text format.

=cut

use warnings;
use strict;
use Data::Dumper;
use DBI;
use POSIX qw(strftime);

## WARNING: type 'chmod 0600 /etc/mysqlbackup' ##
#file must contain 'username:password:host'
#one entry per line. Functionality is similar to /etc/passwd,
#however passwords are stored in plain text and NOT encrypted
my $mysqlbackup_passwd = '/etc/mysqlbackup';

#location of 'mysqldump' program (required)
my $mysqldump = '/usr/bin/mysqldump';

main();

sub main
{
	#check mode of $mysqlbackup_passwd file
	my ($mode) = (stat($mysqlbackup_passwd))[2];
	$mode = sprintf "%04o", $mode & 07777;

	unless (-o $mysqlbackup_passwd && $mode eq '0600')
	{
		die "Please secure '$mysqlbackup_passwd' file. Type 'chmod 0600 $mysqlbackup_passwd'.\n";
	}

	#read in passwords from file
	read_passwd();
}

sub read_passwd
{
	open(PASSWD, $mysqlbackup_passwd) or die "$!";

	while(<PASSWD>)
	{
		chomp;
		my ($user, $pass, $host) = split(/:/);

		#retrieve databases with this user's privileges
		show_databases($user, $pass, $host);
	}

	close(PASSWD);
}

sub show_databases
{
	my ($user, $pass, $host) = @_;
	my $db_list = []; #arrayref to store list of databases

	my $dbh = DBI->connect("dbi:mysql:host=$host", $user, $pass) or die DBI->errstr;

	#execute show databases query
	my $sth = $dbh->prepare("SHOW DATABASES") or die $dbh->errstr;
	$sth->execute() or die $dbh->errstr;

	#fetch results from query
	while (my $db_row = $sth->fetch)
	{
		push(@{$db_list}, $db_row->[0]);
	}

	dump_databases($db_list, $user, $pass, $host);
}

sub dump_databases
{
	my ($db_list, $user, $pass, $host) = @_;
	my $timestamp = strftime "%F-%H.%M", localtime;

	foreach my $db (@{$db_list})
	{
		my $filename = "$host-$db-$timestamp";
		my $dump_cmd = "$mysqldump -u $user -p$pass -h $host --opt $db > $filename.sql";
		my $tar_cmd = "tar czf $filename.tar.gz $filename.sql";
		my $rm_cmd = "rm $filename.sql";

        #tar czf $db.$DATE.tar.gz $FILE
	}
}