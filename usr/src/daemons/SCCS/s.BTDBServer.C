h05335
s 00000/00000/00000
d R 1.2 01/10/20 13:34:56 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/daemons/BTDBServer.C
c Name history : 1 0 src/daemons/BTDBServer.C
e
s 00237/00000/00000
d D 1.1 01/10/20 13:34:55 bmc 1 0
c date and time created 01/10/20 13:34:55 by bmc
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
/*    FILE: BTDBServer.C                                        */
/*    DATE: Wed Oct  5 14:27:58 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTConstants.H"
#include "BTDirs.H"
#include "BTConfigFile.H"
#include "BTDB.H"
#include "BTDBErr.H"
#include "BTSDClient.H"
#include "BTPlayer.H"
#include "BTNetworkEntry.H"
#include "BTGameStats.H"
#include "BTDBServer.H"
#include "btslaved.H"

BTDBServer::BTDBServer()
: netdb_(0), plydb_(0), netlen_(0), netentries_(0),
  plylen_(0), plycnt_(0), plyentries_(0), restart_(0), iter_(queue_)
{
  // We handle memory allocation errors for this in BTDBServer::reload

  netbuf_ = new char [BTDB_NETWORK_BUFSZ];
  plybuf_ = new char [BTDB_PLAYERS_BUFSZ];
}

BTDBServer::~BTDBServer()
{
  BTGameStats *stats;

  if(netdb_)
    delete netdb_;

  if(plydb_)
    delete plydb_;

  if(plybuf_)
    delete [] plybuf_;

  if(netbuf_)
    delete [] netbuf_;

  while(queue_.remove_head(stats))
    delete stats;
}

short BTDBServer::reload()
{
  char pathbuf[1024];
  short err;

  if((netbuf_ == 0) || (plybuf_ == 0))
    return ERRBTDB;	// Out of memory

  if(netdb_)
    delete netdb_;	// We're going to reload the network database

  strcpy(pathbuf, g_conf->datadir());
  strcat(pathbuf, "/");
  strcat(pathbuf, BTDB_NETWORK);

  if((netdb_ = new BTDB(pathbuf, O_CREAT | O_RDWR)) == 0)
    return ERRBTDB;

  if(!(*netdb_)) {
    err = netdb_->error();
    delete netdb_;
    netdb_ = 0;
    return err;
  }

  if(plydb_)		// We're going to reload the player database
    delete plydb_;

  strcpy(pathbuf, g_conf->datadir());
  strcat(pathbuf, "/");
  strcat(pathbuf, BTDB_PLAYERS);

  if((plydb_ = new BTDB(pathbuf, O_CREAT | O_RDWR)) == 0) {
    delete netdb_;
    netdb_ = 0;
    return ERRBTDB;
  }

  if(!(*plydb_)) {
    err = plydb_->error();
    delete netdb_;
    delete plydb_;
    netdb_ = plydb_ = 0;
    return err;
  }

  return ERRBTDBNOERR;
}

short BTDBServer::modify(BTSDClient *client)
{
  BTNetworkEntry *netentry = client->entry();

  if(netdb_->fetch(netentry)) {
    if(netentry->status_ == BTSTATUS_PLAYING)
      netentry->status_ = BTSTATUS_WAITING;
    else
      netentry->status_ = BTSTATUS_PLAYING;

    netdb_->replace(netentry);
    return netdb_->error();
  }

  return netdb_->error();
}

short BTDBServer::enqueue(BTGameStats& stats)
{
  BTGameStats *result = new BTGameStats(stats);

  if(result == 0)
    return ERRBTDB;

  queue_.insert_after_tail(result);
  return ERRBTDBNOERR;
}

int BTDBServer::verify(BTNetworkEntry& entry)
{
  if(netdb_->fetch(&entry)) {
    if(entry.status_ == BTSTATUS_WAITING)
      return 1;
    else
      return 0;
  }

  return 0;
}

short BTDBServer::insert(BTSDClient *client)
{
  BTNetworkEntry *netentry = client->entry();
  BTPlayer player(netentry->userName_);

  plydb_->insert(&player);
  netdb_->insert(netentry);

  return netdb_->error();
}

short BTDBServer::revoke(BTSDClient *client)
{
  netdb_->remove(client->entry());
  return netdb_->error();
}

short BTDBServer::processq()
{
  BTGameStats *stats;
  BTPlayer winner;
  BTPlayer loser;

  short err = ERRBTDBNOERR;

  while(queue_.remove_head(stats)) {
    if(stats == 0)
      continue;

    strncpy(winner.key(), stats->winnerName_, BT_USERNAMELEN);
    strncpy(loser.key(), stats->loserName_, BT_USERNAMELEN);

    if(plydb_->fetch(&winner)) {
      winner.recordWin(stats, &loser);
      plydb_->replace(&winner);
    } else {
      err = plydb_->error();
    }

    if(plydb_->fetch(&loser)) {
      loser.recordLoss(stats, &winner);
      plydb_->replace(&loser);
    } else {
      err = plydb_->error();
    }

    delete stats;
  }

  return err;
}

short BTDBServer::update()
{	
  BTNetworkEntry entry;
  BTPlayer player;
  short err;

  if(restart_) {
    if((err = BTDBServer::reload()) < 0)
      return err;
    restart_ = 0;
  }

  char *bufptr = netbuf_;
  netentries_ = 0;
  netlen_ = 0;

  for(netdb_->rewind(); netdb_->nextrec(&entry); netlen_ += entry.size()) {
    bufptr = entry.writebuf(bufptr);
    netentries_++;
  }

  if(netdb_->error() != ERRBTDBNOERR)
    return netdb_->error();

  if(netlen_ > BTDB_NETWORK_BUFSZ)
    return ERRBTDB;	// The database is bigger than our buffer

  if(plycnt_ == 0) {
    bufptr = plybuf_;
    plyentries_ = 0;
    plylen_ = 0;

    for(plydb_->rewind(); plydb_->nextrec(&player); plylen_ += player.size()) {
      bufptr = player.writebuf(bufptr);
      plyentries_++;
    }

    if(plydb_->error() != ERRBTDBNOERR)
      return plydb_->error();

    if(plylen_ > BTDB_PLAYERS_BUFSZ)
      return ERRBTDB;	// The database is bigger than our buffer
  }

  plycnt_ = (plycnt_ + 1) % BTDB_PLAYERS_CYCLE;
}
E 1
