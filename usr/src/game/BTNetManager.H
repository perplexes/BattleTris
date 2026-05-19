/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTNetManager.H                                      */
/*    DATE: Fri Apr 29 16:36:24 1994                            */
/****************************************************************/

#ifndef BTNETMANAGER_H
#define BTNETMANAGER_H

#include "BTConfig.H"

#include <netdb.h>

#include "StreamSocket.H"
#include "PacketBuffer.H"
#include "BTConstants.H"
#include "BTDBRecord.H"
#include "BTTimeOut.H"

class BTWidget;
class SocketCB;
class BTCommManager;
class BTNetworkEntry;
class BTPlayer;
class BTGameStats;
class BTStartup;

#define BTNETMGR_TIMEOUT 45

#define BTNETMGR_STATUS_UNKNOWN	"Unknown"
#define BTNETMGR_STATUS_WAITING	"Waiting"
#define BTNETMGR_STATUS_PLAYING "Playing"

#define BTNETMGR_USER_WIDTH	8
#define BTNETMGR_HOST_WIDTH	24
#define BTNETMGR_STATUS_WIDTH	8

#define BTNETMGR_NETENTRYLEN	(BTDBRECORD_KEYLEN + BTNETMGR_STATUS_WIDTH + 3)
#define BTNETMGR_PLYENTRYLEN	(BTDBRECORD_KEYLEN + 1)

#define BTNETMGR_SUFFIX		"..."
#define BTNETMGR_SUFFIX_LEN	3

class BTNetManager {

friend class BTStartup;

private:
  BTStartup *startup_;
  BTWidget *widget_;
  SocketCB *netcb_;

  BTCommManager *commMgr_;
  BTCommManager *computer_;

  StreamSocket *daemon_, *sock_, *peer_;
  PacketBuffer dbuf_, pbuf_, tbuf_;

  BTNetworkEntry *entry_;
  char *entrybuf_;
  long entrylen_;

  char hostbuf_[BTDBRECORD_KEYLEN + 1];
  char oppnName_[BTDBRECORD_KEYLEN + 1];

  timeval timeout_;
  int busy_, avail_;

  BTNetworkEntry **netdata_;
  char **netbuf_;
  unsigned long netlen_;

  BTPlayer **plydata_;
  char **plynamebuf_;
  char **plyrankbuf_;
  unsigned long plylen_;

private:
  static void acceptCallback(Socket *sock, void *client_data) {
    ((BTNetManager *) client_data)->acceptCB();
  }

  BT_TIMEOUT_CALLBACK( BTNetManager, challengeCB )

  void fatalErr(short errcode);
  void peerErr(short errcode);

  void acceptCB();
  void challengeCB();

  int verifyEntry(BTNetworkEntry *entry);
  void changeStatus();

  void formatNetworkEntry(int index);
  char *truncateHostName(char *hostName, int width);

public:
  BTNetManager(BTWidget *widget, BTStartup *startup,
	       BTCommManager *commMgr, char *server, unsigned short port);

  ~BTNetManager();

  void challenge(BTNetworkEntry *entry);
  void challengeComputer(int avail = 0);
  void recordStats(int won, BTGameStats *stats);
  void gameOver();

  void setComputer(BTCommManager *computer) {
    computer_ = computer;
    avail_ = 0;
  }

  // Methods to access Network Database

  BTNetworkEntry *netentry(int index);
  void netupdate();
  const char **netbuf() { return (const char **)netbuf_; }
  unsigned long netlen() { return netlen_; }

  // Methods to access Player Database

  BTPlayer *plyentry(char *key);
  void plyupdate();
  const char **plynamebuf() { return (const char **)plynamebuf_; }
  const char **plyrankbuf() { return (const char **)plyrankbuf_; }
  unsigned long plylen() { return plylen_; }

};

#endif
