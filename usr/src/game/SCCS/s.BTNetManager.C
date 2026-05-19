h03797
s 00008/00008/00854
d D 1.3 01/10/24 08:38:08 bmc 4 3
c 1000022 should not resolve server hostname when run with "-X"
e
s 00155/00157/00707
d D 1.2 01/10/21 22:46:37 mws 3 1
c 1000012 derive domain name from Sun RPC domain if DNS is not configured
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:29 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTNetManager.C
c Name history : 1 0 src/game/BTNetManager.C
e
s 00864/00000/00000
d D 1.1 01/10/20 13:35:28 bmc 1 0
c date and time created 01/10/20 13:35:28 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTNetManager.C                                      */
/*    DATE: Fri Apr 29 16:43:35 1994                            */
/****************************************************************/

D 3
#include "BTConfig.H"

E 3
I 3
#include <sys/types.h>
#include <sys/socket.h>
E 3
#include <sys/utsname.h>
I 3
#include <sys/systeminfo.h>
E 3
#include <arpa/inet.h>
D 3

#include <iostream.h>
E 3
I 3
#include <netinet/in.h>
#include <resolv.h>
#include <alloca.h>
#include <ctype.h>
E 3
#include <assert.h>
I 3
#include <unistd.h>
E 3
#include <stdio.h>
#include <pwd.h>

D 3
#include <unistd.h>

// XXX Need #if HAVE_RESOLV_H here
// XXX Sun bug in resolv.h -- MAXDNAME undefined, we don't care
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>

#ifndef MAXDNAME
# define MAXDNAME 1
# include <resolv.h>
# undef MAXDNAME
#endif

#if STDC_HEADERS
# include <stdlib.h>
# include <ctype.h>
#else
# define tolower(x) (((x >= 'A') && (x <= 'Z')) ? (x + 'a' - 'A') : x)
#endif

E 3
I 3
#include "BTConfig.H"
E 3
#include "BattleTris.H"
D 3

E 3
#include "BTNetManager.H"
#include "BTNetworkEntry.H"
#include "BTGameStats.H"
#include "BTPlayer.H"
#include "BTCommManager.H"
#include "BTProtocol.H"
#include "BTMessageDlog.H"
#include "BTStatusDlog.H"
#include "BTStartup.H"
#include "BTDebug.H"
#include "BTXDisplay.H"
#include "XtSocketCB.H"
#include "StreamSocketErr.H"
#include "ParsedFile.H"
#include "BTWidget.H"

BTNetManager::BTNetManager(BTWidget *widget, BTStartup *startup,
D 3
			   BTCommManager *commMgr, char *server,
			   unsigned short port)
: startup_(startup),
  widget_(widget),
  commMgr_(commMgr), busy_(0), avail_(0), entry_(0), entrybuf_(0),
  netdata_(0), netbuf_(0), netlen_(0), peer_(0),
  plydata_(0), plynamebuf_(0), plyrankbuf_(0), plylen_(0)
E 3
I 3
    BTCommManager *commMgr, char *server, unsigned short port)
:	startup_(startup), widget_(widget),
	commMgr_(commMgr), busy_(0), avail_(0), entry_(0), entrybuf_(0),
	netdata_(0), netbuf_(0), netlen_(0), peer_(0),
	plydata_(0), plynamebuf_(0), plyrankbuf_(0), plylen_(0)
E 3
{
D 3
  netcb_ = new XtSocketCB(((BTXDisplay *)DISPLAY)->app_, acceptCallback,
			  this);
E 3
I 3
	netcb_ = new XtSocketCB(((BTXDisplay *)DISPLAY)->app_,
	    acceptCallback, this);
E 3

D 3
  char username[BTDBRECORD_KEYLEN + 1];
  char fullhost[BT_HOSTNAMELEN + 1];
  struct utsname hostinfo;
  struct passwd *pwentry;
  InetAddress addr;
  char *domain;
  short err;
E 3
I 3
	char username[BTDBRECORD_KEYLEN + 1];
	char fullhost[BT_HOSTNAMELEN + 1];
	struct utsname hostinfo;
	struct passwd *pwentry;
	InetAddress addr;
	char *domain = NULL;
	short err;
	long len;
	char c;
E 3

D 3
  timeout_.tv_sec = BTNETMGR_TIMEOUT;
  timeout_.tv_usec = 0;
E 3
I 3
	timeout_.tv_sec = BTNETMGR_TIMEOUT;
	timeout_.tv_usec = 0;
E 3

D 3
  sock_ = new StreamSocket(addr);
E 3
I 3
	sock_ = new StreamSocket(addr);
E 3

D 3
  memset((void *) oppnName_, 0, sizeof(oppnName_));
E 3
I 3
	memset(oppnName_, 0, sizeof(oppnName_));
E 3

D 3
  if((pwentry = getpwuid(getuid())) == NULL) {
    cerr << "BattleTris: Failed to lookup uid in password file" << endl;
    bt_terminate(1);
  }
E 3
I 3
	if ((pwentry = getpwuid(getuid())) == NULL) {
		cerr << "BattleTris: user ID " << getuid() << " is unknown\n";
		bt_terminate(1);
	}
E 3

D 3
  strcpy(username, pwentry->pw_name);
  uname(&hostinfo);
  strcpy(fullhost, hostinfo.nodename);
E 3
I 3
	(void) strncpy(username, pwentry->pw_name, BTDBRECORD_KEYLEN);
	username[BTDBRECORD_KEYLEN] = '\0';
	(void) uname(&hostinfo);
	(void) strncpy(fullhost, hostinfo.nodename, BT_HOSTNAMELEN);
	fullhost[BT_HOSTNAMELEN] = '\0';
E 3

#ifdef _PATH_RESCONF
D 3
  ParsedFile rconf("/etc/resolv.conf");
E 3
I 3
	ParsedFile rconf("/etc/resolv.conf");
E 3
#else
D 3
  ParsedFile rconf(_PATH_RESCONF);
E 3
I 3
	ParsedFile rconf(_PATH_RESCONF);
E 3
#endif
I 3
	/*
	 * If we can find a resolv.conf file to parse, see if we can pick up
	 * the domain name from there.  Otherwise use the Sun RPC domain if
	 * there is one.  We also work around silly old Sun ENS NIS domains.
	 */
	while (!(rconf.eof() || rconf.fail())) {
		rconf.parseline();
		if (rconf.ntokens() > 1 &&
		    strcmp(rconf.token(), "domain") == 0) {
			domain = rconf.token();
			break;
		}
	}
E 3

D 3
  while(!(rconf.eof() || rconf.fail())) {
    rconf.parseline();
    if((rconf.ntokens() > 1) && (strcmp(rconf.token(), "domain") == 0)) {
       domain = rconf.token();
       strcat(fullhost, ".");
       strcat(fullhost, domain);
       strcat(username, "@");
       strcat(username, domain);
       break;
    }
  }
E 3
I 3
	if (domain == NULL && (len = sysinfo(SI_SRPC_DOMAIN, &c, 1)) > 0) {
		domain = (char *) alloca(len);
		(void) sysinfo(SI_SRPC_DOMAIN, domain, len);
		if (strcmp(domain, "sunsoft.eng.sun.com") == 0)
			(void) strcpy(domain, "eng.sun.com");
	}
E 3

D 3
  for(char *ptr = fullhost; *ptr; ptr++)
    *ptr = tolower(*ptr);
E 3
I 3
	if (domain != NULL) {
		(void) strcat(fullhost, ".");
		(void) strcat(fullhost, domain);
		(void) strcat(username, "@");
		(void) strcat(username, domain);
	}
E 3

D 3
  sock_->installCB(netcb_, SOCKET_CB_READ);
E 3
I 3
	for (char *ptr = fullhost; *ptr; ptr++)
		*ptr = tolower(*ptr);

	sock_->installCB(netcb_, SOCKET_CB_READ);
E 3
  
D 3
  InetAddress boundaddr;
  if((err = sock_->listen(SOMAXCONN, boundaddr)) < 0)
    fatalErr(err);
E 3
I 3
	InetAddress boundaddr;
	if ((err = sock_->listen(SOMAXCONN, boundaddr)) < 0)
		fatalErr(err);
E 3

D 3
  entry_ = new BTNetworkEntry(username, fullhost, boundaddr);
  entrylen_ = entry_->size();
  entrybuf_ = new char [entrylen_];
  entry_->writebuf(entrybuf_);
E 3
I 3
	entry_ = new BTNetworkEntry(username, fullhost, boundaddr);
	entrylen_ = entry_->size();
	entrybuf_ = new char [entrylen_];
	entry_->writebuf(entrybuf_);
E 3

D 3
  InetAddress daemonAddr(port, server);
E 3
I 3
D 4
	InetAddress daemonAddr(port, server);
E 3

D 3
  if(!daemonAddr) {
    cerr << "BattleTris: Failed to resolve address for host " << server << endl;
    bt_terminate(1);
  }
E 3
I 3
	if (!daemonAddr) {
		cerr << "BattleTris: Failed to resolve address for host " <<
		    server << endl;
		bt_terminate(1);
	}
E 3

E 4
D 3
  InetAddress localAddr;
  daemon_ = new StreamSocket(localAddr);
  dbuf_.socket(daemon_);
E 3
I 3
	InetAddress localAddr;
	daemon_ = new StreamSocket(localAddr);
	dbuf_.socket(daemon_);
E 3

D 3
  if(g_resources.no_server) {
    cout << "BattleTris: Network Manager disabled for "
         << username << endl << flush;
    return;
  }
E 3
I 3
	if (g_resources.no_server) {
		cout << "BattleTris: Network Manager disabled for " <<
		    username << endl << flush;
		return;
	}
E 3

I 4
	InetAddress daemonAddr(port, server);

	if (!daemonAddr) {
		cerr << "BattleTris: Failed to resolve address for host " <<
		    server << endl;
		bt_terminate(1);
	}

E 4
D 3
  cout << "BattleTris: Connecting to server " << server 
       << " at port " << port << " ...\n" << flush;
E 3
I 3
	cout << "BattleTris: Connecting to server " << server <<
	    " at port " << port << " ...\n" << flush;
E 3

D 3
  if((err = daemon_->connect(daemonAddr, localAddr)) < 0)
    fatalErr(err);
E 3
I 3
	if ((err = daemon_->connect(daemonAddr, localAddr)) < 0)
		fatalErr(err);
E 3

D 3
  entry_->addrnet_ = localAddr.net();
  entry_->addrlna_ = localAddr.lna();
  entry_->writebuf(entrybuf_);
E 3
I 3
	entry_->addrnet_ = localAddr.net();
	entry_->addrlna_ = localAddr.lna();
	entry_->writebuf(entrybuf_);
E 3

D 3
  if((err = dbuf_.recvpacket()) < 0)
    fatalErr(err);
E 3
I 3
	if ((err = dbuf_.recvpacket()) < 0)
		fatalErr(err);
E 3

D 3
  if(dbuf_.datatype() != BT_ACCEPTED) {
    cerr << "BattleTris: Server daemon rejected connection request" << endl;
    bt_terminate(1);
  }
E 3
I 3
	if (dbuf_.datatype() != BT_ACCEPTED) {
		cerr << "BattleTris: Server daemon rejected connection "
		    "request" << endl;
		bt_terminate(1);
	}
E 3
    
D 3
  if((err = dbuf_.sendpacket(BT_QUER_CONN, entrylen_, entrybuf_) < 0)) {
    cerr << "BattleTris: Failed to transmit network entry to server" << endl;
    fatalErr(err);
  }
E 3
I 3
	if ((err = dbuf_.sendpacket(BT_QUER_CONN, entrylen_, entrybuf_) < 0)) {
		cerr << "BattleTris: Failed to transmit network entry "
		    "to server" << endl;
		fatalErr(err);
	}
E 3

D 3
  cout << "BattleTris: Network Manager enabled for " << username << endl;

E 3
I 3
	cout << "BattleTris: Network Manager enabled for " << username << endl;
E 3
#ifndef NDEBUG
D 3
  cout << "BattleTris: Listening on port " << entry_->port_ << endl;
E 3
I 3
	cout << "BattleTris: Listening on port " << entry_->port_ << endl;
E 3
#endif
D 3

E 3
}

BTNetManager::~BTNetManager()
{
D 3
  int i;
E 3
I 3
	int i;
E 3

D 3
  if(netdata_ != 0) {
    for(i = 0; i < netlen_; i++)
      delete netdata_[i];
    delete [] netdata_;
  }
E 3
I 3
	if (netdata_ != 0) {
		for (i = 0; i < netlen_; i++)
			delete netdata_[i];
		delete [] netdata_;
	}
E 3

D 3
  if(netbuf_ != 0) {
    for(i = 0; i < netlen_; i++)
      delete [] netbuf_[i];
    delete [] netbuf_;
  }
E 3
I 3
	if (netbuf_ != 0) {
		for (i = 0; i < netlen_; i++)
			delete [] netbuf_[i];
		delete [] netbuf_;
	}
E 3

D 3
  if(plydata_ != 0) {
    for(i = 0; i < plylen_; i++)
      delete plydata_[i];
    delete [] plydata_;
  }
E 3
I 3
	if (plydata_ != 0) {
		for (i = 0; i < plylen_; i++)
			delete plydata_[i];
		delete [] plydata_;
	}
E 3

D 3
  if(plynamebuf_ != 0) {
    for(i = 0; i < plylen_; i++)
      delete [] plynamebuf_[i];
    delete []  plynamebuf_;
  }
E 3
I 3
	if (plynamebuf_ != 0) {
		for (i = 0; i < plylen_; i++)
			delete [] plynamebuf_[i];
		delete []  plynamebuf_;
	}
E 3

D 3
  if(plyrankbuf_ != 0) {
    for(i = 0; i < plylen_; i++)
      delete [] plyrankbuf_[i];
    delete [] plyrankbuf_;
  }
E 3
I 3
	if (plyrankbuf_ != 0) {
		for (i = 0; i < plylen_; i++)
			delete [] plyrankbuf_[i];
		delete [] plyrankbuf_;
	}
E 3

D 3
  if(sock_ != 0) delete sock_;
  if(peer_ != 0) {
    commMgr_->sock_ = 0;
    delete peer_;
  }
E 3
I 3
	if (sock_ != 0)
		delete sock_;

	if (peer_ != 0) {
		commMgr_->sock_ = 0;
		delete peer_;
	}
E 3
    
I 3
	if (entry_ != 0)
		delete entry_;
E 3

D 3
  if(entry_ != 0)
    delete entry_;
E 3
I 3
	if (entrybuf_ != 0)
		delete [] entrybuf_;
E 3

D 3
  if(entrybuf_ != 0)
    delete [] entrybuf_;
E 3
I 3
	if (daemon_) {
		dbuf_.sendpacket(BT_DISCONNECT);
		sleep(2);
		delete daemon_;
	}
E 3

D 3
  if(daemon_) {
    dbuf_.sendpacket(BT_DISCONNECT);
    sleep(2);
    delete daemon_;
  }
  delete netcb_;
E 3
I 3
	delete netcb_;
E 3
}

void BTNetManager::challenge(BTNetworkEntry *entry)
{
  char opponentName[32];
  BTPlayer *player = 0;

  if(busy_) {
    BTDebug("Aborting challenge because incoming challenge detected");
    return;
  }

  busy_ = 1;

  if((entry->addrnet_ == entry_->addrnet_) &&
     (entry->addrlna_ == entry_->addrlna_) &&
     (entry->port_ == entry_->port_)) {
    BTMessageDlog errMsg(widget_, "This is no time to play with yourself.");
    busy_ = 0;
    return;
  }

  if(entry->status_ != BTSTATUS_WAITING) {
    BTMessageDlog errMsg(widget_, "You may only challenge those who are waiting.");
    busy_ = 0;
    return;
  }

  if(!BTNetManager::verifyEntry(entry)) {
    BTMessageDlog errMsg(widget_, "BattleTris server reports this user is no longer available to challenge");	
    busy_ = 0;
    return;
  }

  InetAddress myAddr;
  short err;

  peer_ = new StreamSocket(myAddr);
  pbuf_.socket(peer_);

  InetAddress destAddr(entry->port_, entry->addrnet_, entry->addrlna_);

  if((err = peer_->connect(destAddr)) < 0) {
    BTMessageDlog errMsg(widget_, "Challenge aborted...Unable to connect to specified host.");
    peerErr(err);
    return;
  }

  BTDebug("Sending challenge to peer");

  if((err = pbuf_.sendpacket(BT_CHALL, entrylen_, entrybuf_)) < 0) {
    BTMessageDlog errMsg(widget_, "Challenge aborted because a network error occurred.");
    peerErr(err);
    return;
  }

  BTDebug("Waiting for response from peer");

  if(peer_->ready(timeout_)) {
    if((err = pbuf_.recvpacket()) < 0) {
      BTMessageDlog errMsg(widget_, "Challenge aborted because a network error occurred.");
      peerErr(err);
      return;
    }

    switch(pbuf_.datatype()) {

    case BT_ACCPT: {
      BTDebug("Challenge accepted");
      BTNetManager::changeStatus(); // Change to BTSTATUS_PLAYING

      if((err = pbuf_.sendpacket(BT_START)) < 0) {
	BTMessageDlog errMsg(widget_, "Challenge aborted because a network error occurred.");
	peerErr(err);
	return;
      }

      startup_->showGame();

      BTPlayer player(entry->userName_);
      if(!player.plan(opponentName, sizeof(opponentName), 0, 0))
	strcpy(opponentName, entry->userName_);

	BTDebug("Starting game against" << opponentName);
	if(commMgr_->startGame(peer_, opponentName)) {

	  sock_->removeCB(SOCKET_CB_READ);
	  return;

	} else {

	  BTDebug("Timed out waiting to start game");

	  delete peer_;
	  peer_ = 0;
	  busy_ = 0;

	  BTNetManager::changeStatus(); // Change to BTSTATUS_WAITING
	}

	break;
      }

    case BT_DENY: {
      BTDebug("Challenge was rejected");
      BTMessageDlog msgDlog(widget_, "Opponent wimped out and rejected challenge.");
      break;
    }

    case BT_BUSY: {
      BTDebug("Opponent is currently busy");
      BTMessageDlog msgDlog(widget_, "Opponent is currently busy receiving a challenge.");
      break;
    }

    default: {
      BTDebug("Bogus token received from peer");
      BTMessageDlog msgDlog(widget_, "Challenge aborted because of an invalid response.");
    }

    }
  } else {
    BTDebug("Challenge timed out");
    BTMessageDlog msgDlog(widget_, "Challenge timed out...No response from opponent.");
  }

  if(peer_ != 0) {
    delete peer_;
    peer_ = 0;
  }

  busy_ = 0;
}

void BTNetManager::challengeComputer(int avail)
{

  if (!(avail_ = avail)) {
    busy_ = 1;
    BTNetManager::changeStatus(); // Change to BTSTATUS_PLAYING
    sock_->removeCB(SOCKET_CB_READ);
  } 
  peer_ = 0;

  startup_->showGame();
  commMgr_->startGame(computer_);
}

void BTNetManager::recordStats(int won, BTGameStats *stats)
{
  // If the same user account played itself from two different nodes,
  // then don\'t bother recording any results

  if(strcmp(entry_->userName_, oppnName_) == 0)
    return;

  if(won) {
    strcpy(stats->winnerName_, entry_->userName_);
    strcpy(stats->loserName_, oppnName_);
  } else {
    strcpy(stats->winnerName_, oppnName_);
    strcpy(stats->loserName_, entry_->userName_);
  }

  char *buf = new char [stats->datalen()];
  short err;

  stats->writebuf(buf);

  if((err = dbuf_.sendpacket(BT_QUER_RESULT, stats->datalen(), buf)) < 0) {
    if(err == ERRSTREAMBROKEN) {
      cerr << "BattleTris: Lost connection to server" << endl;
      delete [] buf;
      fatalErr(err);
    } else {
      BTMessageDlog errMsg(widget_, "Warning: A network error occurred.");
      cerr << "BattleTris: " << StreamSocketErrMsg(err) << endl;
      perror("BattleTris");
    }
  }

  delete [] buf;
}

void BTNetManager::gameOver()
{
  BTDebug("Network Manager registering game over");
  sock_->installCB(netcb_, SOCKET_CB_READ);

  if(avail_)
    avail_ = 0;
  else
    BTNetManager::changeStatus(); // Change to BTSTATUS_WAITING

  if(peer_ != 0) {
    delete peer_;
    peer_ = 0;
  }
    
  busy_ = 0;
}

void BTNetManager::acceptCB(void)
{
  static char oppnhost[BT_HOSTNAMELEN + 1];
  char buf[sizeof(BTNetworkEntry)];
  StreamSocket *tclient;
  BTNetworkEntry pentry;
  char *bufptr;
  short err;

  if(busy_) {
    BTDebug("Accepting connection while busy");

    if(sock_->accept(tclient) < 0)
      return;

    tbuf_.socket(tclient);

    if(tbuf_.recvpacket() < 0)
      return;

    BTDebug("Notifying client we're busy");
    tbuf_.sendpacket(BT_BUSY);
    delete tclient;
    return;
  }

  busy_ = 1;

  BTDebug("Accepting connection while not busy");

  if((err = sock_->accept(peer_)) < 0) {
    BTMessageDlog errMsg(widget_, "Incoming challenge aborted because a network error occurred.");
    peerErr(err);
    return;
  }

  pbuf_.socket(peer_);

  if((err = pbuf_.recvpacket()) < 0) {
    BTMessageDlog errMsg(widget_, "Incoming challenge aborted because a network error occurred.");
    peerErr(err);
    return;
  }

  if(pbuf_.datatype() == BT_CHALL) {
    pentry.readbuf(pbuf_.databuf());

    strncpy(oppnName_, pentry.userName_, BT_USERNAMELEN);
    strncpy(oppnhost, pentry.hostName_, BT_HOSTNAMELEN);
    if((bufptr = strchr(oppnhost, '.')) != NULL)
      *bufptr = '\0';

D 3
/*
    startup_->challenge(oppnName_,
			truncateHostName(oppnhost, BT_HOSTABBRLEN));
			*/
E 3
    BTPlayer *player = plyentry(pentry.userName_);
    if ( ! player )
      player = new BTPlayer(pentry.userName_);
    startup_->challenge(player);

    XtAppAddTimeOut(((BTXDisplay *)DISPLAY)->app_,
		    100, challengeCB_CB, this);
    return;
  }

  BTDebug("Invalid packet received");  
  delete peer_;
  peer_ = 0;
  busy_ = 0;
}

void BTNetManager::challengeCB(unsigned long *)
{
  short err;

  switch(startup_->accepted()) {

  case 1: {
    BTDebug("Responding to client");

    if((err = pbuf_.sendpacket(BT_ACCPT)) < 0) {
      BTMessageDlog errMsg(widget_, "Incoming challenge aborted because a network error occurred.");
      peerErr(err);
      return;
    }
      

    if((err = pbuf_.sendpacket(BT_START)) < 0) {
      BTMessageDlog errMsg(widget_, "Incoming challenge aborted because a network error occurred.");
      peerErr(err);
      return;
    }

    BTPlayer player(oppnName_);
    char opponentName[BT_NICKNAMELEN + 1];
    if(!player.plan(opponentName, sizeof(opponentName), 0, 0))
      strcpy(opponentName, oppnName_);

    BTNetManager::changeStatus(); // Change to BTSTATUS_PLAYING
    startup_->showGame();

    if(commMgr_->startGame(peer_, opponentName)) {
      sock_->removeCB(SOCKET_CB_READ);
      return;
    } else {
      BTDebug("Timed out waiting to start game");
      delete peer_;
      peer_ = 0;
      busy_ = 0;
    }
    
    break;
  }

  case 0: {
    BTDebug("Responding to client");

    if((err = pbuf_.sendpacket(BT_DENY)) < 0) {
      BTMessageDlog errMsg(widget_, "Incoming challenge aborted because a network error occurred.");
      peerErr(err);
      return;
    }

    delete peer_;
    peer_ = 0;
    busy_ = 0;

    break;
  }

  case -1: {
    XtAppAddTimeOut(((BTXDisplay *)DISPLAY)->app_,
		    100, challengeCB_CB, this);
    break;
  }

  }
}

int BTNetManager::verifyEntry(BTNetworkEntry *entry)
{
  unsigned short valid;
  short err;

  if((err = dbuf_.sendpacket(BT_QUER_VERIFY, BTDBRECORD_KEYLEN + 1,
			     entry->key())) < 0) {
    if(err == ERRSTREAMBROKEN) {
      cerr << "BattleTris: Connection to server broke" << endl;
      fatalErr(err);
    } else {
      return 0;
    }
  }

  if((err = dbuf_.recvpacket()) < 0) {
    if(err == ERRSTREAMBROKEN) {
      cerr << "BattleTris: Connection to server broke" << endl;
      fatalErr(err);
    } else {
      return 0;
    }
  }

  if(dbuf_.datatype() == BT_RESP_VERIFY) {
    valid = *((unsigned short *) dbuf_.databuf());
    valid = ntohs(valid);

    if(valid) {
      BTDebug("Daemon verified entry as good");
      return 1;
    } else {
      BTDebug("Daemon verified entry as bad");
      return 0;
    }

  } else {
    cerr << "BattleTris: Invalid packet received from server" << endl;
    return 0;
  }
}

void BTNetManager::changeStatus()
{
  short err;

  if((err = dbuf_.sendpacket(BT_QUER_UPDATE)) < 0) {
    if(err == ERRSTREAMBROKEN) {
      cerr << "BattleTris: Connection to server broke" << endl;
      fatalErr(err);
    } else {
      cerr << "BattleTris: Network status may not have been updated" << endl;
      return;
    }
  }
}

BTNetworkEntry *BTNetManager::netentry(int index)
{
  if((index < 0) || (index >= netlen_))
    return (BTNetworkEntry *) 0;

  return netdata_[index];
}

void BTNetManager::netupdate()
{
  char *bufptr;
  short err;
  int i;

  unsigned long oldlen = netlen_;

  if((err = dbuf_.sendpacket(BT_QUER_NETDB)) < 0)
    goto neterr;

  if((err = dbuf_.recvpacket()) < 0)	// BT_RESP_DBLEN packet
    goto neterr;

  netlen_ = *((unsigned long *) dbuf_.databuf());
  netlen_ = ntohl(netlen_);

  if((err = dbuf_.recvpacket()) < 0)	// BT_RESP_NETDB packet
    goto neterr;

  if(netdata_ != 0) {
    for(i = 0; i < oldlen; i++)
      delete netdata_[i];
    delete [] netdata_;
  }

  if(netbuf_ != 0) {
    for(i = 0; i < oldlen; i++)
      delete [] netbuf_[i];
    delete [] netbuf_;
  }

  if(netlen_ == 0)
    return;

  netdata_ = new BTNetworkEntry * [netlen_];
  bufptr = dbuf_.databuf();

  for(i = 0; i < netlen_; i++) {
    netdata_[i] = new BTNetworkEntry;
    bufptr = netdata_[i]->readbuf(bufptr);
  }

  if(netlen_ > 0) {
D 3
    qsort((void *) netdata_, netlen_, sizeof(BTNetworkEntry *),
	  BTNetworkEntry::compare);
E 3
I 3
    qsort(netdata_, netlen_, sizeof(BTNetworkEntry *), BTNetworkEntry::compare);
E 3

    netbuf_ = new char * [netlen_];

    for(i = 0; i < netlen_; i++) {
      netbuf_[i] = new char [BTNETMGR_NETENTRYLEN];
      BTNetManager::formatNetworkEntry(i);
    }
  }

  return;

 neterr:
  if(err == ERRSTREAMBROKEN) {
    cerr << "BattleTris: Connection to server broke" << endl;
    fatalErr(err);
  } else {
    cerr << "BattleTris: Update of network database failed" << endl;
    return;
  }
}

void BTNetManager::formatNetworkEntry(int index)
{
  char *abbr = truncateHostName(netdata_[index]->hostName_,
				BTNETMGR_HOST_WIDTH);

  const char *status;

  switch(netdata_[index]->status_) {
  case BTSTATUS_WAITING:
    status = BTNETMGR_STATUS_WAITING;
    break;
  case BTSTATUS_PLAYING:
    status = BTNETMGR_STATUS_PLAYING;
    break;
  default:
    status = BTNETMGR_STATUS_UNKNOWN;
  }

  sprintf(netbuf_[index], "%-*.*s %-*s %-*s", BTNETMGR_USER_WIDTH,
	  BTNETMGR_USER_WIDTH, netdata_[index]->userName_,
	  BTNETMGR_HOST_WIDTH, abbr,
	  BTNETMGR_STATUS_WIDTH, status);
}

BTPlayer *BTNetManager::plyentry(char *name)
{
  assert(name != 0);

  // try to update db if empty
  if ( plydata_ == 0 )
    plyupdate();

  if(plydata_ == 0)
    return (BTPlayer *) 0;

  BTPlayer *key = new BTPlayer(name);

  BTPlayer **found = (BTPlayer **)
    bsearch((const void *) &key, (const void *) plydata_, plylen_,
	    sizeof(BTPlayer *), BTPlayer::compareName);

  delete key;

  if(found != 0)
    return *found;

  return (BTPlayer *) 0;
}

void BTNetManager::plyupdate()
{
  char *bufptr;
  short err;
  int i;

  unsigned long oldlen = plylen_;

  if((err = dbuf_.sendpacket(BT_QUER_PLYDB)) < 0)
    goto neterr;

  if((err = dbuf_.recvpacket()) < 0)	// BT_RESP_DBLEN packet
    goto neterr;

  plylen_ = *((unsigned long *) dbuf_.databuf());
  plylen_ = ntohl(plylen_);

  if((err = dbuf_.recvpacket()) < 0)	// BT_RESP_PLYDB packet
    goto neterr;

  if(plydata_ != 0) {
    for(i = 0; i < oldlen; i++)
      delete plydata_[i];
    delete [] plydata_;
  }

  if(plynamebuf_ != 0) {
    for(i = 0; i < oldlen; i++)
      delete [] plynamebuf_[i];
    delete [] plynamebuf_;
  }

  if(plyrankbuf_ != 0) {
    for(i = 0; i < oldlen; i++)
      delete [] plyrankbuf_[i];
    delete [] plyrankbuf_;
  }

  if(plylen_ == 0)
    return;

  plydata_ = new BTPlayer * [plylen_];
  bufptr = dbuf_.databuf();

  for(i = 0; i < plylen_; i++) {
    plydata_[i] = new BTPlayer;
    bufptr = plydata_[i]->readbuf(bufptr);
  }

  if(plylen_ > 0) {
    plyrankbuf_ = new char * [plylen_];
    plynamebuf_ = new char * [plylen_];

    qsort((void *) plydata_, plylen_, sizeof(BTPlayer *),
	  BTPlayer::compareRank);

    for(i = 0; i < plylen_; i++) {
      plyrankbuf_[i] = new char [BTNETMGR_PLYENTRYLEN];
      strcpy(plyrankbuf_[i], plydata_[i]->key());
    }

    qsort((void *) plydata_, plylen_, sizeof(BTPlayer *),
	  BTPlayer::compareName);

    for(i = 0; i < plylen_; i++) {
      plynamebuf_[i] = new char [BTNETMGR_PLYENTRYLEN];
      strcpy(plynamebuf_[i], plydata_[i]->key());
    }
  }

  return;

 neterr:
  if(err == ERRSTREAMBROKEN) {
    cerr << "BattleTris: Connection to server broke" << endl;
    fatalErr(err);
  } else {
    cerr << "BattleTris: Update of player database failed" << endl;
    return;
  }
}

char *BTNetManager::truncateHostName(char *hostName, int width)
{
  if(strlen(hostName) > width) {
    strncpy(hostbuf_, hostName, width - BTNETMGR_SUFFIX_LEN);
    strncpy(&(hostbuf_[width - BTNETMGR_SUFFIX_LEN]),
	    BTNETMGR_SUFFIX, BTNETMGR_SUFFIX_LEN);

    return hostbuf_;
  }

  return hostName;
}

void BTNetManager::fatalErr(short errcode)
{
  if(errcode < 0) {
    cerr << "BattleTris: " << StreamSocketErrMsg(errcode) << endl;
    perror("BattleTris");
    cerr << "BattleTris: Fatal error occurred" << endl;
    bt_terminate(1);
  }
}

void BTNetManager::peerErr(short errcode)
{
  if(errcode < 0) {
    if(peer_ != 0) {
      delete peer_;
      peer_ = 0;
    }

    busy_ = 0;
  }
}
E 1
