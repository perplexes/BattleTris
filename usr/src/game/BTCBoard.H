/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTCBoard.H                                          */
/*    ASGN:                                                     */
/*    DATE: Tue May  2 02:28:51 1995                            */
/****************************************************************/

#ifndef BTCBOARD_H
#define BTCBOARD_H

#include "BTConfig.H"
#include "BTPiece.H"
#include "BTBoardManager.H"
#include "BTConstants.H"
#include "BTDebug.H"

#define BT_ORIENT_NO 6

enum HoleType { BTC_NOHOLE, BTC_OCCUPIED, BTC_POCCUPIED, BTC_OPEN,
		BTC_CLOSED, BTC_UNKNOWN };

class BTWeaponManager;
struct BTCBoard;

struct BTCPenalties {
  int chp_, ohp_, cvhp_, hp_, vp_, lb_, hap_;
  int midline_;
};

struct BTCBox {
  BTCBox() : hole_(BTC_UNKNOWN), checked_(0), being_checked_(0) {};
  void assign( int x, int y, BTCBoard *board) {
    x_ = x;
    y_ = y;
    board_ = board;
  }
  int x_, y_;
  int checked_, being_checked_;
  BTCBoard *board_;
  void reset() {
    checked_ = being_checked_ = 0;
    hole_ = BTC_UNKNOWN;
  }
  int hasBottom();
  HoleType hole_;
  BTCBox &operator=( BTCBox &box ) {
    checked_ = box.checked_;
    being_checked_ = box.being_checked_;
    hole_ = box.hole_;
    return *this;
  }
  HoleType holeCheck( int );
};


struct BTCBoard {
  BTCBoard() : open_holes_(0), closed_holes_(0) {
    int x,y;

    for ( y = 0 ; y < BT_BOARD_HGT ; y++ )
      for ( x = 0 ; x < BT_BOARD_WTH ; x++ )
	box_[x][y].assign( x, y, this );
    for ( x = 0 ; x < BT_BOARD_WTH ; x++ )
      ptops_[x] = tops_[x] = BT_BOARD_HGT;
  }
  BTCBox box_[BT_BOARD_WTH][BT_BOARD_HGT];
  int top_, open_holes_, closed_holes_;
  int tops_[BT_BOARD_WTH], ptops_[BT_BOARD_WTH];
  int h_holes_[BT_BOARD_WTH];
  int v_holes_[BT_BOARD_HGT];
  int covered_holes_, lines_cleared_;
  int isHole( int, int );
  float variance();
  void reset();
  void rescan (BTBoardManager *);
  void drawMaps(BTBoardManager * = NULL, BTPiece * = NULL, int = 0, int = 0);
  float eval( int, int, BTPiece *, BTWeaponManager *, int, int,
	      BTCPenalties, BTCBoard *baseline = 0, int ceiling = 0,
	      int floor = BT_BOARD_HGT - 1 );
  float variance_,
  cov_hole_pen_,
  height_pen_,
  line_bonus_,
  value_,
  hole_pen_;
  
};

struct BTPosition {

  int pos_[BT_BOARD_WTH+BT_PIECE_WIDTH][BT_BOARD_HGT][BT_ORIENT_NO];

  BTPosition() {
    reset();
  }
  void reset() {
    for ( int y = 0 ; y < BT_BOARD_HGT ; y++ )
      for ( int x = 0 ; x < BT_BOARD_WTH+BT_PIECE_WIDTH ; x++ )
	for ( int o = 0 ; o < BT_ORIENT_NO ; o++ )
	  pos_[x][y][o] = 0;
  }
  void check( int x, int y, int orientation ){
    if ( x < 0 )
      x = BT_BOARD_WTH - x - 1;
    if ( (y < 0 ) || ( y >= BT_BOARD_HGT ) ||
	 ( x > (BT_BOARD_WTH+BT_PIECE_WIDTH-1) ) )
      BTDebug( "Shouldn\'t be checking here!" );
    pos_[x][y][orientation % BT_ORIENT_NO] = 1;
  }
  
  int unchecked( int x, int y, int orientation ) {
    if ( x < 0 )
      x = BT_BOARD_WTH - x - 1;
    if ( (y < 0 ) || ( y >= BT_BOARD_HGT ) ||
	 ( x > (BT_BOARD_WTH+BT_PIECE_WIDTH-1) ) )
      return 0;
    return ! pos_[x][y][orientation % BT_ORIENT_NO];
  }

};

struct LineStat {
  int closed_holes_;
  int open_holes_;
  int occupied_;
};
#endif


