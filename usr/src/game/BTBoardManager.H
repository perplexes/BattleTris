/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTBoardManager.H                                    */
/*    ASSN:                                                     */
/*    DATE: Tue Feb  8 21:21:01 1994                            */
/****************************************************************/

#ifndef BTBOARDMANAGER_H
#define BTBOARDMANAGER_H

#define BT_BOTTLE_X 3
#define BT_BOTTLE_Y 4

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "BTRingNode.H"
#include "BTWeaponManager.H"
#include "BTBoard.H"

class BTBoxManager;
class BTBoard;
class BTBox;

class BTBoardManager : public BTRingNode {
friend class BTPieceManager;
friend class BTBoard;
friend class BTPiece;
private:
  BTBox ***map_;
  BTBox *new_fill_[BT_PIECE_HEIGHT*BT_PIECE_WIDTH];
  BTWeaponManager *weapon_manager_;
  int width_, height_;
  int idiot_, upside_;
  int computer_;
  int actives_, fill_count_, old_lines_;
  short reason_;
  void swap (int,int,int,int);
  void removeLine (int line, int x1, int x2);
  void insertLine ();
  void flipOnHoriz();  
  void flipOnVert();

public:
  BTBoxManager *box_manager_;

  BTBoardManager (BTWeaponManager *weapon,
		  int width = BT_BOARD_WTH, int height = BT_BOARD_HGT,
		  int computer = 0);
  ~BTBoardManager();
  inline int occupied (int x, int y); 
  void fill (int x, int y, BTBox *new_box);
  void setIdiot(short reason = 0) { idiot_ = 1; reason_ = reason; }
  void flushIdiot() { if (idiot_) send (BT_IDIOT, &reason_); idiot_ = 0;}
  int checkLines();
  void landed (int x, int y);
  void redraw();
  void clear();
  void receive (BTRingPacket *packet);
  void newBoard (BTBoard *board);
};

inline int BTBoardManager::occupied (int x, int y) {
  if (!weapon_manager_->BTActive[BT_FALL_OUT]) {
    if (x < 0 || x >= width_ || y >= height_ || y < 0) return 1;
    if (map_[x][y]) return 1;
    return 0;
  } 
  if (x < 0 || x >= width_ || (y >= height_ && 
     (x < BT_FALL_OUT_LEDGE || x >= width_- BT_FALL_OUT_LEDGE)) || (y < 0 && 
     (x < BT_FALL_OUT_LEDGE || x>= width_- BT_FALL_OUT_LEDGE))) 
       return 1;
  if (y < height_ && y >= 0)
    if (map_[x][y]) return 1;
  if (y < 0 - BT_PIECE_HEIGHT || y > height_ + BT_PIECE_HEIGHT) 
    return 1;
  return 0;
}
  
#endif
