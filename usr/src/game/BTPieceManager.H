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
