# Class: nginx::params
#
# Parameters for and from the nginx module.
#
# Parameters :
#  none
#
# Sample Usage :
#  include nginx::params
#
class nginx::params {
  # The easy bunch
  $service = 'nginx'
  $confdir = '/etc/nginx'
  # user
  case $::operatingsystem {
    'Debian',
    'Ubuntu': { $user = 'www-data' }
    default:  { $user = 'nginx' }
  }
  # package
  case $::operatingsystem {
    'Gentoo': { $package = 'www-servers/nginx' }
    default:  { $package = 'nginx' }
  }
  # service restart
  case $::operatingsystem {
    'Fedora',
    'RedHat',
    'CentOS': { $service_restart = '/sbin/service nginx reload' }
    default:  { $service_restart = '/etc/init.d/nginx reload' }
  }
  # remove_default_conf
  case $::operatingsystem {
    'Fedora',
    'RedHat',
    'CentOS': { $remove_default_conf = true }
    default:  { $remove_default_conf = false }
  }
  # include /etc/nginx/sites-enabled/*
  case $::operatingsystem {
    'Debian',
    'Ubuntu': { $sites_enabled = true }
    default:  { $sites_enabled = false }
  }
  # modular 1.10+
  case $::operatingsystem {
    'Fedora': {
      if versioncmp($::operatingsystemrelease, '24') >= 0 {
        $modular = true
      } else {
        $modular = false
      }
    }
    'RedHat','CentOS': {
      if versioncmp($::operatingsystemrelease, '8') >= 0 {
        $modular = true
      } else {
        $modular = false
      }
    }
    default:  { $modular = false }
  }
}
