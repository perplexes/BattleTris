h58586
s 00000/00000/00000
d R 1.2 01/10/20 13:35:25 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTCBoard.C
c Name history : 1 0 src/game/BTCBoard.C
e
s 00457/00000/00000
d D 1.1 01/10/20 13:35:24 bmc 1 0
c date and time created 01/10/20 13:35:24 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTCBoard.C                                          */
/*    ASGN:                                                     */
/*    DATE: Tue May  2 05:00:15 1995                            */
/****************************************************************/


#include "BTWeaponManager.H"
#include "BTCBoard.H"

#define BT_HOLE_DECAY 0.50
#define BT_MAX_VARIANCE 1750.0
#define BT_MIN_HOLE_PEN_FRAC 0.10

HoleType BTCBox::holeCheck( int scan_top ) {
    HoleType top, left, right;
    if ( being_checked_ )
      return BTC_UNKNOWN;
    if ( checked_ )
      return hole_;
    being_checked_ = 1;
    if ( (hole_ == BTC_OCCUPIED) || (hole_ == BTC_POCCUPIED) ) {
      checked_ = 1;
      being_checked_ = 0;
      return hole_;
    }
    // if we\'re at the top, we\'re not a hole
    if ( y_ <= scan_top ) {
      hole_ = BTC_NOHOLE;
      checked_ = 1;
      being_checked_ = 0;
      return hole_;
    }
    // check box above
    top = (board_->box_[x_][y_-1]).holeCheck( scan_top );
    switch (top) {
    case BTC_NOHOLE:
    case BTC_OPEN:
      hole_ = top;
      checked_ = 1;
      being_checked_ = 0;
      return hole_;
      break;
    default:
      break;
    }
    // we\'re either an open or closed hole
    if ( x_ > 0 ) {
      left = (board_->box_[x_-1][y_]).holeCheck( scan_top );
      switch (left) {
      case BTC_NOHOLE:
      case BTC_OPEN:
	hole_ = BTC_OPEN;
	being_checked_ = 0;
	checked_ = 1;
	return hole_;
	break;
      default:
	break;
      }
    }
    else
      left = BTC_OCCUPIED;
    if ( x_ < (BT_BOARD_WTH-1) ) {
      right = (board_->box_[x_+1][y_]).holeCheck( scan_top );
      switch (right) {
      case BTC_NOHOLE:
      case BTC_OPEN:
	hole_ = BTC_OPEN;
	being_checked_ = 0;
	checked_ = 1;
	return hole_;
	break;
      default:
	break;
      }
    }
    else
      right = BTC_OCCUPIED;
    if ( right == BTC_UNKNOWN ) {
      being_checked_ = 0;
      checked_ = 1;
      hole_ = BTC_UNKNOWN;
      return hole_;
    }
    hole_ = BTC_CLOSED;
    being_checked_ = 0;
    checked_ = 1;
    return hole_;
  }

int BTCBox::hasBottom() {
  if ( (hole_ == BTC_OCCUPIED) || (hole_ == BTC_POCCUPIED) )
    return 1;
  if ( y_ < BT_BOARD_HGT-1 )
    return (board_->box_[x_][y_+1]).hasBottom();
  return 0;
}
    
float BTCBoard::variance() {

    float temp = 0.0;
    unsigned int temp2;
    int j, prev_height = ptops_[1];
    
    for ( j = 0 ; j < BT_BOARD_WTH ; j ++ ) {
/*
      temp2 = abs(BT_BOARD_HGT-ptops_[j]-prev_height);
      temp2 *= temp2;
      temp += temp2;
      prev_height = BT_BOARD_HGT-ptops_[j];
      */
      temp2 = abs(ptops_[j]-prev_height);
      temp2 *= temp2;
      temp += temp2;
      prev_height = ptops_[j];
    }
    temp += temp2; // account for furthest column
    return temp;
}

void BTCBoard::rescan( BTBoardManager *board ) {
    top_ = BT_BOARD_HGT;
    int x,y;

    for ( x = 0 ; x < BT_BOARD_WTH ; x++ )
      tops_[x] = BT_BOARD_HGT;
    for ( y = BT_BOARD_HGT - 1; y >= 0; y-- ) {
      v_holes_[y] = 0;
      for ( x = 0 ; x < BT_BOARD_WTH ; x++ ) {
	h_holes_[x] = 0;
	box_[x][y].reset();
	if ( board->occupied(x,y) ) {
	  box_[x][y].hole_ = BTC_OCCUPIED;
	  top_ = tops_[x] = y;
	}
      }
    }
    closed_holes_ = open_holes_ = 0;
  }

void BTCBoard::reset() {
    for ( int y = BT_BOARD_HGT - 1; y >= 0; y-- )  
      for ( int x = 0 ; x < BT_BOARD_WTH ; x++ )
	if ( box_[x][y].hole_ != BTC_OCCUPIED )
	  box_[x][y].checked_ = 0;
}

void BTCBoard::drawMaps( BTBoardManager *board, BTPiece *piece, int x, int y ) {
    int i,j;

/*
    for ( j = 0 ; j < BT_BOARD_HGT ; j++ ) {
      for ( i = 0 ; i < BT_BOARD_WTH ; i++ ) {
        if ( board && board->occupied(i,j) )
          cout << "* ";
	else if ( piece && (j >= y) && (y<j+BT_PIECE_HEIGHT) && (i>=x) &&
	     (i<x+BT_PIECE_WIDTH) && piece->isMapped(i-x,j-y) )
	  cout << "* ";
        else cout << "- ";
     }
     cout << endl << endl;
    }
    */
     for ( j = 0 ; j < BT_BOARD_HGT ; j++ ) {
        for ( i = 0 ; i < BT_BOARD_WTH ; i++ )
	  switch (box_[i][j].hole_) {
	  case BTC_OPEN:
	    cout << "O ";
	    break;
	  case BTC_CLOSED:
	    cout << "C ";
	    break;
	  case BTC_OCCUPIED:
	    cout << "* ";
	    break;
	  case BTC_POCCUPIED:
	    cout << "P ";
	    break;
	  case BTC_UNKNOWN:
	    cout << "U ";
	    break;
	  default:
	    cout << ". ";
	    break;
	  }
	cout << endl;
      }
  }

float BTCBoard::eval( int i, int j, BTPiece *piece,
		      BTWeaponManager *weapon_manager, int no_tetri,
		      int no_nice_day,
		      BTCPenalties penalties, BTCBoard *baseline,
		      int ceiling, int floor )
{
  closed_holes_ = open_holes_ = covered_holes_ = lines_cleared_ = 0;
  int x,y, near_death = 0;
  int piece_left = -1, piece_right = -1, piece_top = BT_PIECE_HEIGHT, piece_bottom = 0;
  static int blocks[BT_BOARD_WTH][BT_BOARD_HGT];

  int avoid_x1 = BT_DEFAULT_X - BT_PIECE_WIDTH / 2 + 1,
    avoid_x2 =  BT_DEFAULT_X + BT_PIECE_WIDTH / 2 - 1,
    avoid_y2 = BT_PIECE_HEIGHT / 2;

  for ( x = 0 ; x < BT_BOARD_WTH ; x++ ) {
    h_holes_[x] = 0;
    ptops_[x] = tops_[x];
  }
  // We gotta know if we\'ve got the FORCE
  int force = weapon_manager->BTActive[BT_FORCE];

  if ( piece ) {
    // Map piece onto our board
    for ( y = j ; (y < j+BT_PIECE_HEIGHT) && (y < BT_BOARD_HGT) ; y++ )
      for ( x = i ; (x < i+BT_PIECE_WIDTH) && (x < BT_BOARD_WTH) ; x++ )
	if ( piece->isMapped(x-i, y-j) ) {
	  if ( y < avoid_y2 && x >= avoid_x1 && x <= avoid_x2 )
	    near_death = 1;
	  box_[x][y].hole_ = BTC_POCCUPIED;
	  if ( y + y - j < top_ )
	    top_ = y + y - j;
	  if ( y < ptops_[x] )
	    ptops_[x] = y;
	  if ( (y-j) < piece_top )
	    piece_top = y-j;
	  if ( (y-j) > piece_bottom )
	    piece_bottom = y-j;
	  if ( piece_left == -1 )
	    piece_left = x-i;
	  else if ( piece_left > x-i )
	    piece_left = x-i;
	  if ( x-i > piece_right )
	    piece_right = x-i;
	}

    // Do an initial scan of the board to check the heights of columns,
    // see if any lines were cleared and, if so, account for them

    for ( y = 0 ; y < BT_BOARD_HGT ; y++ ) {
      int line_cleared = 1;
      for ( x = 0 ; x < BT_BOARD_WTH ; x++ ) {
	if ( y )
	  blocks[x][y] = blocks[x][y-1];
	else
	  blocks[x][0] = 0;
	switch ( box_[x][y].hole_ ) {
	case BTC_OCCUPIED:
	case BTC_POCCUPIED:
	  blocks[x][y]++;
	  break;
	default:
	  line_cleared = 0;
	  break;
	}
      }
      // Now, we\'ll drop the board down if there\'s a line cleared and
      // loop until no more lines are cleared
      if ( line_cleared ) {
//	line_cleared = 0;
	lines_cleared_++;
	top_++;
	int k, l, x1 = 0, x2 = BT_BOARD_WTH;
	if (weapon_manager->BTActive[BT_BOTTLE]) 
      		if ((y <= BT_BOARD_HGT / 2 + BT_BOTTLE_Y) && 
		    (y >= BT_BOARD_HGT / 2 - BT_BOTTLE_Y)) {  
		  x1 = BT_BOTTLE_X;
		  x2 = BT_BOARD_WTH - BT_BOTTLE_X;
		}
	// Adjust the tops of columns
        for ( k = x1 ; k < x2 ; k++ ) {
	  if ( (ptops_[k] < y) && !force )
	    ptops_[k]++;
	  else if ( ptops_[k] == y ) {
	    ptops_[k] = BT_BOARD_HGT;
	    for ( int w = y ; (w < BT_BOARD_HGT) ; w++ )
	      if ( (box_[k][w].hole_ == BTC_OCCUPIED) ||
		   (box_[k][w].hole_ == BTC_POCCUPIED) ) {
		ptops_[k] = w;
		break;
	      }
	  }
	}
	// Drop the boxes down to replace cleared line
	if ( !force )
	  for ( l = y ; l >= 0 ; l-- )
	    for ( k = x1 ; k < x2 ; k++ ) {
	      if ( l ) {
		box_[k][l] = box_[k][l-1];
		blocks[k][l] = blocks[k][l-1];
	      }
	      else {
		box_[k][l].reset();
		blocks[k][l] = 0;
	      }
	    }
	else
	  // We don\'t drop the boxes down if the force is
	  // active.
	  for ( k = 0 ; k < BT_BOARD_WTH ; k++ ) {
	    box_[k][y].reset();
	    blocks[k][y] == blocks[k][y-1];
	  }
      y--;
      }
    }
  }
  
  float fraction  = 0.0,
    decay;
  
  hole_pen_ = 0.0;
  cov_hole_pen_ = 0.0;
  float closed_pen_ = 0.0, open_pen_ = 0.0;

  for ( y = BT_BOARD_HGT - 1; y >= ceiling; y-- ) {
    v_holes_[y] = 0;
    decay = 1.0;
    if ( j+piece_top <= y )
      for ( int d = 0; d < y-j-piece_top; d++ )
	decay *= BT_HOLE_DECAY;
    else
      decay = 0.0;
    for ( x = 0 ; x < BT_BOARD_WTH ; x++ ) {
      switch (box_[x][y].holeCheck( ceiling )) {
      case BTC_OPEN:
	if ( !weapon_manager->BTActive[BT_FALL_OUT] ||
	     (weapon_manager->BTActive[BT_FALL_OUT] &&
	      box_[x][y].hasBottom()) ) {
	  if ( y <= floor ) {
	    open_pen_ += penalties.ohp_; // * decay;
	    open_holes_++;
	  }
	}
	break;
      case BTC_CLOSED:
	if ( weapon_manager->BTActive[BT_FALL_OUT] ) {
	  if ( ! box_[x][y].hasBottom() )
	    continue;
	}
	if ( y <= floor ) {
	  h_holes_[x]++;
	  v_holes_[y]++;
//	  if ( baseline && (!baseline->v_holes_[y]) ) {
	    closed_holes_++;
	    closed_pen_ += penalties.chp_; // *decay;
//	  }
	}
	if ( piece && (y >= j+piece_top) && (x >= i+piece_left) &&
	     (x <= i+piece_right) ) {
//	  if ( baseline && (baseline->box_[x][y].hole_ == BTC_CLOSED )) {
		  covered_holes_++;
		  cov_hole_pen_ += decay * blocks[x][y];
//		}
	}
	break;
      default:
	break;
      }
    }
  }
  
  if ( baseline )
    fraction = (float)baseline->top_ / (float)BT_BOARD_HGT;
  else
    fraction = (float)top_ / (float)BT_BOARD_HGT;
  
  variance_ = variance() * penalties.vp_ * (1 - fraction) * (1 - fraction) *
    (1 - fraction);

  cov_hole_pen_ *= (float)penalties.cvhp_;

  // If Ernie has the force and clears a line, let\'s not punish
  // for open holes
  if ( force )
    penalties.ohp_ /= 2;

  if ( baseline )
    hole_pen_ = (closed_holes_ - baseline->closed_holes_) * penalties.chp_ +
      (open_holes_ - baseline->open_holes_) * penalties.ohp_;
  else
    hole_pen_ = closed_pen_ + open_pen_;

  if ( baseline && (baseline->variance_ > (BT_MAX_VARIANCE/(force+1)) )) {
    hole_pen_ *= BT_MIN_HOLE_PEN_FRAC;
    cov_hole_pen_ *= BT_MIN_HOLE_PEN_FRAC;
  }

  // compute line bonus
  if ( lines_cleared_ ) {
    if ( hole_pen_ > 0.0 ) {
      for ( int m = j+piece_top ; (m <= j+piece_bottom) &&
	    (m < BT_BOARD_HGT) ; m ++ ) {
	if ( v_holes_[m] ) {
	  // verify that the piece did not create the holes
	  if ( !baseline || (baseline->v_holes_[m] == v_holes_[m]) )
	    no_tetri = 1;
	}
      }
    }
    if ( piece->isHappy() ) {
      if ( no_nice_day ) {
	line_bonus_ = 0 - penalties.hap_;
      }
      else
	line_bonus_ = penalties.hap_;
    }
    else if ( no_tetri || ( top_ < penalties.midline_ ) )
      line_bonus_ = (float) penalties.lb_ * lines_cleared_ * (1 - fraction);
    
    else
      line_bonus_ = (float) penalties.lb_ *
	(float) (-4.0 + (float) lines_cleared_) * fraction;
  }
  else
    line_bonus_ = 0.0;
  
  if ( piece_top == BT_PIECE_HEIGHT )
    piece_top = 0;

  fraction = 1.0 - (float)(j+piece_top) / (float)BT_BOARD_HGT;
  height_pen_ = fraction * fraction * (float)penalties.hp_;
/*
  float midline = (float)penalties.midline_ / (float)BT_BOARD_HGT;
  if ( fraction <=  midline )
     height_pen_ = fraction * (float)penalties.hp_;
  else {
     fraction = 1.0 - ((float)(j+piece_top) / (float)penalties.midline_);
     height_pen_ = (fraction * fraction * + midline) * (float)penalties.hp_;
  }
  */
  
  value_ = variance_ + cov_hole_pen_ + hole_pen_ + height_pen_ -
    line_bonus_;

    // Square the value if we\'re not going to be able to place the next
    // piece
  if (near_death)
    value_ *= value_;

  return value_;
  
}


int BTCBoard::isHole( int x, int y ) {
  switch (box_[x][y].holeCheck( 0 )) {
  case BTC_CLOSED:
    return 1;
    break;
  default:
    return 0;
    break;
  }
}
E 1
