h40490
s 00000/00000/00000
d R 1.2 01/10/20 13:35:30 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTPieceManager.H
c Name history : 1 0 src/game/BTPieceManager.H
e
s 00048/00000/00000
d D 1.1 01/10/20 13:35:29 bmc 1 0
c date and time created 01/10/20 13:35:29 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTPieceManager.H                                    */
/*    ASSN:                                                     */
/*    DATE: Tue Feb  8 21:36:18 1994                            */
/****************************************************************/

#ifndef BTPIECEMANAGER_H
#define BTPIECEMANAGER_H

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "BTRingNode.H"
#include "BTConstants.H"

class BTPiece;
class BTGame;
class BTPixmap;
class BTBoxManager;
class BTBoardManager;
class BTWidget;

class BTPieceManager : public BTRingNode {
friend class BTBoardManager;
private:
  double keep_prob_[BT_MAX_PIECES+1];
  BTPiece *piece_[BT_MAX_PIECES+1];
  BTBoxManager *box_manager_; 
  BTBoardManager *board_;
  int hap_on_;
  int broken_, old_piece_;

public:
  BTPieceManager(BTWidget *da, BTBoardManager *board,
		 BTPixmap *gimp = NULL);
  ~BTPieceManager();
  BTPiece *create (int x, int y);
  void dispose (BTPiece *old);
  void receive (BTRingPacket *packet);
};


#endif
E 1
