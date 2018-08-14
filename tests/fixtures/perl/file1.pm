package DDG::Publisher;
# ABSTRACT: Generation of the static files of DuckDuckGo and its microsites.

use MooX;
use Path::Class;
use Class::Load ':all';
use IO::All -utf8;
use HTML::Packer;
use JSON;
use File::Path qw(make_path);

=attr site_classes

List of classes that should get executed on publishing.

=cut

has site_classes => (
	is => 'ro',
	lazy => 1,
	builder => 1,
);

sub _build_site_classes {[qw(
	Duckduckgo
	Donttrackus
	Dontbubbleus
	Duckduckhack
)]}

=attr extra_template_dirs

List of extra directions that should be used for templates.

=cut

has extra_template_dirs => (
	is => 'ro',
	lazy => 1,
	builder => 1,
);

sub _build_extra_template_dirs {[qw(
	templates
)]}


has cache_dir => (
    is => 'ro',
    lazy => 1,
    builder => 1,
);


sub _build_cache_dir {
    my $dir = $ENV{DDG_PUBLISHER_CACHE_DIR} ? $ENV{DDG_PUBLISHER_CACHE_DIR} : $ENV{HOME}."/publisher";
    make_path($dir) unless -d $dir;
    return $dir;
}


=attr sites

This attribute holds the objects of the site classes that should get build.

=cut

has sites => (
	is => 'ro',
	lazy => 1,
	builder => 1,
);

sub _build_sites {
	my ( $self ) = @_;
	return {map {
		my $class = 'DDG::Publisher::Site::'.$_;
		load_class($class);
		s/([a-z])([A-Z])/$1_$2/g;
		$_ = lc($_);
		lc($_) => $class->new( key => lc($_), publisher => $self );
	} @{$self->site_classes}};
}

=attr compression

See L<DDG::App::Publisher/compression>.

=cut

has compression => (
	is => 'ro',
	default => sub { 0 },
);

=attr dryrun

See L<DDG::App::Publisher/dryrun>.

=cut

has dryrun => (
	is => 'ro',
	predicate => 1,
);

=attr quiet

Don't print Text::XSlate warnings

=cut

has quiet => (
	is => 'ro',
	predicate => 1,
);

sub BUILD {
	my ( $self ) = @_;
	$self->sites;
}

=method publish_to

This method it called to publish the files to the given specific directory.

=cut

sub publish_to {
	my ( $self, $target ) = @_;
	my $target_dir = dir($target)->absolute;
	$target_dir->mkpath unless -d "$target_dir";
	my $count = 0;
	my $packer;
	$packer = HTML::Packer->init() if ($self->compression);

	#
	# For every site...
	#

	for my $site (values %{$self->sites}) {

		#
		# Generate a datafile for the site, which can be used for deeper
		# processing of the static files. (It's used by the internal code
		# of DDG to generate, for example, the nginx config)
		#

		my $data_file = file($target_dir,$site->key.'.json')->absolute;
		io($data_file)->print(encode_json($site->save_data));
	};
	return $count;
}

1;