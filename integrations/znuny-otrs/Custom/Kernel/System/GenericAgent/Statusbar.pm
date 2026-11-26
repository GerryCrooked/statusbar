# --
# Statusbar for Znuny / OTRS – Generic Agent module.
# A ticket that matches the criteria and is open shows a message on users' desktops; when it is
# closed, merged or removed – or no longer matches – the message gets its green all-clear.
#
# Generic Agent job: "Execute Custom Module" = Kernel::System::GenericAgent::Statusbar, parameters:
#   URLFile      file with the Statusbar webhook URL (first line), readable by the otrs user
#   (or URL      the webhook URL itself – visible to every admin of the job)
#   MinPriority  priority ID from which a ticket counts, default 5 ("5 very high")
#   Queue        only tickets in this queue (full name, e.g. "IT::Outages")      – optional
#   Type         only tickets of this type (e.g. "Incident::Major")              – optional
# Tickets that Statusbar opened itself (first article contains "statusbar-origin") are skipped, so
# they never come back as a second message.
# See integrations/znuny-otrs/README.md in the Statusbar repository.
# --

package Kernel::System::GenericAgent::Statusbar;

use strict;
use warnings;
use utf8;

use LWP::UserAgent;

our @ObjectDependencies = (
    'Kernel::Config',
    'Kernel::System::JSON',
    'Kernel::System::Log',
    'Kernel::System::Ticket',
    'Kernel::System::Ticket::Article',
);

sub new {
    my ( $Type, %Param ) = @_;
    return bless {}, $Type;
}

sub Run {
    my ( $Self, %Param ) = @_;

    my $LogObject = $Kernel::OM->Get('Kernel::System::Log');
    my %Config    = %{ $Param{New} || {} };

    my %Ticket = $Kernel::OM->Get('Kernel::System::Ticket')->TicketGet(
        TicketID => $Param{TicketID},
        UserID   => 1,
    );
    return if !%Ticket;

    # Opened by Statusbar ("Statusbar -> Znuny"): nothing to report back.
    my $ArticleObject = $Kernel::OM->Get('Kernel::System::Ticket::Article');
    my ($First) = $ArticleObject->ArticleList(
        TicketID  => $Param{TicketID},
        OnlyFirst => 1,
    );
    if ($First) {
        my %Article = $ArticleObject->BackendForArticle( %{$First} )->ArticleGet(
            TicketID  => $Param{TicketID},
            ArticleID => $First->{ArticleID},
        );
        return 1 if ( $Article{Body} // '' ) =~ m{statusbar-origin};
    }

    my $URL = $Config{URL} // '';
    if ( !$URL && $Config{URLFile} ) {
        if ( open my $Fh, '<', $Config{URLFile} ) {
            $URL = <$Fh> // '';
            close $Fh;
        }
        else {
            $LogObject->Log( Priority => 'error', Message => "Statusbar: cannot read $Config{URLFile}: $!" );
            return;
        }
    }
    $URL =~ s/\s+//g;
    if ( !$URL ) {
        $LogObject->Log( Priority => 'error', Message => 'Statusbar: parameter URL or URLFile missing' );
        return;
    }

    my $MinPriority = $Config{MinPriority} // ( ( $Config{Queue} || $Config{Type} ) ? 0 : 5 );
    my $Matches = $Ticket{PriorityID} >= $MinPriority;
    $Matches = 0 if $Config{Queue} && $Ticket{Queue} ne $Config{Queue};
    $Matches = 0 if $Config{Type}  && ( $Ticket{Type} // '' ) ne $Config{Type};
    my $Open = $Ticket{StateType} !~ m{^(?:closed|removed|merged)$};

    my %Body = (
        key      => "znuny-$Ticket{TicketNumber}",
        status   => ( $Matches && $Open ) ? 'problem' : 'ok',
        host     => $Ticket{Queue},
        service  => $Ticket{Title},
        title    => $Ticket{Title},
        severity => $Ticket{PriorityID} >= 5 ? 'error' : $Ticket{PriorityID} == 4 ? 'warning' : 'info',
    );
    my $JSON = $Kernel::OM->Get('Kernel::System::JSON')->Encode( Data => \%Body );
    utf8::encode($JSON) if utf8::is_utf8($JSON);

    my $UserAgent = LWP::UserAgent->new( timeout => 15, agent => 'Znuny-Statusbar' );
    my $Proxy     = $Kernel::OM->Get('Kernel::Config')->Get('WebUserAgent::Proxy');
    $UserAgent->proxy( [ 'http', 'https' ], $Proxy ) if $Proxy;

    my $Response = $UserAgent->post(
        $URL,
        'Content-Type' => 'application/json; charset=utf-8',
        Content        => $JSON,
    );
    if ( !$Response->is_success ) {
        $LogObject->Log(
            Priority => 'error',
            Message  => "Statusbar: ticket $Ticket{TicketNumber}: " . $Response->status_line . ' '
                . ( $Response->decoded_content // '' ),
        );
        return;
    }
    $LogObject->Log(
        Priority => 'notice',
        Message  => "Statusbar: $Body{key} $Body{status}: " . ( $Response->decoded_content // '' ),
    );
    return 1;
}

1;
