h62898
s 00001/00001/00085
d D 1.2 01/10/23 19:29:17 bmc 3 1
c 1000019 Contrary to cgh's wishes, Ernie has more names than "Greased"
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:26 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTCommManager.H
c Name history : 1 0 src/game/BTCommManager.H
e
s 00086/00000/00000
d D 1.1 01/10/20 13:35:25 bmc 1 0
c date and time created 01/10/20 13:35:25 by bmc
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
/*    FILE: BTCommManager.H                                     */
/*    DATE: Wed Apr 13 16:55:05 1994                            */
/****************************************************************/

#ifndef BTCOMMMANAGER_H
#define BTCOMMMANAGER_H

#include "BTConfig.H"
#include "List.H"
#include "BTRingNode.H"
#include "BTWeapon.H"
#include "BTScore.H"
#include "BTBoard.H"
#include "BTArsenal.H"
#include "BTPimp.H"
#include "StreamSocket.H"
#include "PacketBuffer.H"

class SocketCB;
class BTStartup;
class BTComputer;
class BTWidget;

class BTCommManager : public BTRingNode {

friend class BTGame;
friend class BTNetManager;

private:
  StreamSocket *sock_;
  PacketBuffer pbuf_;
  SocketCB *gamecb_;
  BTWidget *widget_;
  List<BTWeaponToken> weapq_;
  List<BTRingPacket *> stash_;
  BTBoard *board_buf_;
  BTPimp *pimp_;	// The street-walkin\' pimp
  BTCommManager *cain_;	// The evil brother CommManager
  int able_;
  BTStartup *startup_;
  BTComputer *computer_;

private:
  static void gameCallback(Socket *sock, void *client_data) {
    ((BTCommManager *) client_data)->gameCB();
  }

  void fatalErr(short err);
  void gameCB();

  void recvScore();
  void recvWeapon();
  void recvBoard();
  void recvArsenal();

public:
  void sendScore(BTScore *score);
  void sendWeapon(BTWeapon *weapon);
  void sendBoard(BTBoard *board);
  void sendArsenal(BTArsenal *arsenal);

  BTCommManager(BTWidget *widget, BTPimp *pimp, 
		BTStartup *startup);

  ~BTCommManager();

  int startGame(StreamSocket *sock, char *opponentName);
  int startGame(BTCommManager *sibling);

  void gameOver();
  void flushWeapons();

  void receive(BTRingPacket *packet);
  void receiveFromLocal(BTRingPacket *packet);
  void receiveFromSibling(BTRingPacket *packet);

D 3
  void setComputer( BTComputer * );
E 3
I 3
  void setComputer(BTComputer *computer) { computer_ = computer; }
E 3
  void flushStash();
  void clear();
};

#endif

E 1
