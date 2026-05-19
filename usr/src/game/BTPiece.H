/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTPiece.H                                           */
/*    ASSN:                                                     */
/*    DATE: Sun Feb  6 23:17:54 1994                            */
/****************************************************************/

#ifndef BTPIECE_HEADER
#define BTPIECE_HEADER

#include "BTConfig.H"
#include "BTConstants.H"

//
// BTPiece
//
// Each piece has a piece map of boxes making up the actual piece (the
// map_).  In addition, it has an instance variable, rot_, which specifies
// how much of the map should be rotated upon a rotate request.  The
// rotate method is virtual, and some subclasses actually override it 
// (this is how the wall piece can have cool rotation).
//

class BTBox;
class BTBoardManager;

class BTPiece {
friend class BTGame;
friend class BTComputer;
protected:
  int x_, y_, color_, rot_, placed_;
  BTBox *map_[BT_PIECE_WIDTH][BT_PIECE_HEIGHT];
  BTBoardManager *board_;
  int orientation_;

public:
  BTPiece(int color, BTBoardManager *board);
  
  int isMapped (int x, int y);
  virtual int isHappy() { return 0; };

  int moveTo (int x, int y);
  int canMoveTo (int x, int y);

  virtual int rotate ( int redraw = 1, int reverse = 0 );
  virtual void construct (int x, int y) {};
  int canRotate( int, int );
  void reset();
  void resetOrientation();
  
  virtual void redraw();
  virtual void landed();
  virtual ~BTPiece();

  int orientations_;
};

class BTElPiece : public BTPiece {
public:
  BTElPiece (BTBoardManager *box_manager);
  void construct (int x, int y);
  ~BTElPiece(){};
};

class BTRevElPiece : public BTPiece {
public:
  BTRevElPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTRevElPiece(){};
};

class BTSldLftPiece : public BTPiece { 
public:
  BTSldLftPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTSldLftPiece(){};
};

class BTSldRtPiece : public BTPiece { 
public:
  BTSldRtPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTSldRtPiece(){};
};

class BTLongPiece : public BTPiece { 
public:
  BTLongPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTLongPiece(){};
};

class BTPlugPiece : public BTPiece { 
public:
  BTPlugPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTPlugPiece(){};
};

class BTBoxPiece : public BTPiece { 
public:
  BTBoxPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTBoxPiece(){};
};

class BTDiePiece : public BTPiece { 
public:
  BTDiePiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTDiePiece(){};
};

class BTHappyPiece : public BTPiece { 
public:
  BTHappyPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTHappyPiece(){};
  virtual int isHappy() { return 1; };
};

// Feared Weird pieces
class BTDogPiece : public BTPiece { 
public:
  BTDogPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTDogPiece(){};
};

class BTRevDogPiece : public BTPiece { 
public:
  BTRevDogPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTRevDogPiece(){};
};

class BTCapPiece : public BTPiece { 
public:
  BTCapPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTCapPiece(){};
};

class BTWallPiece : public BTPiece { 
private:
  int state_;
public:
  BTWallPiece (BTBoardManager *board);
  virtual int rotate( int redraw = 1, int reverse = 0 );
  void construct (int x, int y);
  ~BTWallPiece(){};
};

class BTTowerPiece : public BTPiece { 
public:
  BTTowerPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTTowerPiece(){};
};

class BTStarPiece : public BTPiece { 
private:
  int state_;
public:
  BTStarPiece (BTBoardManager *board);
  virtual int rotate( int redraw = 1, int reverse = 0 );
  void construct (int x, int y);
  ~BTStarPiece(){};
};

class BTWeirdLongPiece : public BTPiece { 
private:
  int state_;
public:
  BTWeirdLongPiece (BTBoardManager *board);
  virtual int rotate( int redraw = 1, int reverse = 0 );
  void construct (int x, int y);
  ~BTWeirdLongPiece(){};
};

class BTFourByFourPiece : public BTPiece { 
public:
  BTFourByFourPiece (BTBoardManager *board);
  void construct (int x, int y);
  ~BTFourByFourPiece(){};
};

class BTLongDongPiece : public BTPiece {
public:
  BTLongDongPiece(BTBoardManager *board);
  void construct (int x, int y);
  ~BTLongDongPiece(){};
}; 

#endif
