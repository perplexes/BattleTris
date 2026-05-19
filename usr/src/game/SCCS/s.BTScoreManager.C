h52640
s 00000/00000/00000
d R 1.2 01/10/20 13:35:32 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTScoreManager.C
c Name history : 1 0 src/game/BTScoreManager.C
e
s 00220/00000/00000
d D 1.1 01/10/20 13:35:31 bmc 1 0
c date and time created 01/10/20 13:35:31 by bmc
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
/*    FILE: BTScoreManager.C                                    */
/*    ASSN:                                                     */
/*    DATE: Sun Feb 20 21:32:13 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTScoreManager.H"

#define BT_SCORE_WIDTH 325
#define BT_SCORE_HEIGHT 210
#define BT_MONDALE_RATE .30
#define BT_LINES_TIL_BAZ 20

BTScoreManager::BTScoreManager(BTWeaponManager *weapon, BTRecon *recon, 
			       BTWidget *parent, int x, int y) 
: recon_ (recon), BTRingNode(), weapon_manager_(weapon) {
  spy_on_ = 0;
  tax_on_ = 0;
  max_funds_ = 0;
  max_op_funds_ = 0;
  keating_ = 0;
  lines_til_baz_ = BT_LINES_TIL_BAZ;
  if (parent) {
    widget_rep_ = new BTTextWidget (parent, "score_box", x, y, 
				    x + BT_SCORE_WIDTH + 1, 
				    y + BT_SCORE_HEIGHT + 1);
    widget_rep_->manage();
    widget_rep_->setTab (20);
    widget_rep_->setText (0, 0, "Your score: ");
    widget_rep_->setText (1, 0, "Opponent's score: ");
    widget_rep_->setText (3, 0, "Your lines:  ");
    widget_rep_->setText (4, 0, "Opponent's lines: ");

    widget_rep_->setText (6, 0, "Your funds: ");
    
    widget_rep_->setText (8, 0, "Lines 'til bazaar: ");
    
    
  } else widget_rep_ = 0;
}

void BTScoreManager::updateDisplay() {
  char s[20];
  char *base = s;

  if (widget_rep_) {
    sprintf (s, "%d     ", (int) rep_.score_);
    widget_rep_->setText (0, 1, s);
    sprintf (s, "%d      ", (int) rep_.op_score_);
    widget_rep_->setText (1, 1, s);
    sprintf (s, "%d       ", (int) rep_.lines_);
    widget_rep_->setText (3, 1, s);
    sprintf (s, "%d       ", (int) rep_.op_lines_);
    widget_rep_->setText (4, 1, s);
    sprintf (s, "%d       ", (int) rep_.funds_);
    widget_rep_->setText (6, 1, s);

    if (spy_on_ && recon_) {
      sprintf (s, "%d      ", (int) recon_->adjustFunds (rep_.op_funds_));
      widget_rep_->setText (7, 1, s);
      sprintf (s, "%d      ", (int) lines_til_baz_);
      widget_rep_->setText (9, 1, s);
      sprintf (s, "         ");
      widget_rep_->setText (8, 1, s);
    } else {
      sprintf (s, "%d       ", (int) lines_til_baz_);
      widget_rep_->setText (8, 1, s);
    }
  }
}

void BTScoreManager::receive (BTRingPacket *packet) {
  need_update_ = 0;

  switch (packet->token) {
  case BT_START: 
    keating_ = spy_on_ = tax_on_ = 0;
    rep_.clear();
    lines_til_baz_ = BT_LINES_TIL_BAZ;
    if (widget_rep_) {
      widget_rep_->setText (9, 0, "                             ");
      widget_rep_->setText (7, 0, "                             ");
      widget_rep_->setText (8, 0, "Lines 'til bazaar: ");
    }
    updateDisplay();
    max_funds_ = max_op_funds_ = 0;
    break;

  case BT_WPN_LAUNCH: {
    BTWeapon *wpn = (BTWeapon *) packet->data;
    switch (wpn->token()) {
    case BT_AMES:
    case BT_ACE:
    case BT_CONDOR:
      if ( widget_rep_ ) {
	widget_rep_->setText (8, 0, "                              ");
	widget_rep_->setText (7, 0, "Opponent's funds: ");
	widget_rep_->setText (9, 0, "Lines 'til bazaar: ");
      }
      updateDisplay();
      break;

    case BT_MONDALE:
      tax_on_ += wpn->duration();
      break;

    case BT_KEATING:
      keating_ = rep_.op_funds_;
      break;
    }
    break;
  }

  case BT_WPN_ON: {
    BTWeapon *wpn = (BTWeapon *) packet->data;
    switch (wpn->token()) {
    case BT_KEATING:
      rep_.funds_ = 0;
      break;
      
    case BT_REAGAN:
      rep_.funds_ *= -1;
      break;
    }
    break;
  }

  case BT_SCORE: {
    rep_.score_ += *((short *) packet->data);
    need_update_ = 1;
    break;
  }

  case BT_CONDOR_OFF: {
    if (widget_rep_) {
      widget_rep_->setText (9, 0, "                             ");
      widget_rep_->setText (7, 0, "                             ");
      widget_rep_->setText (8, 0, "Lines 'til bazaar: ");
      spy_on_ = 0;
    }
    break;
  }
  case BT_OP_SCORE: {
    BTScore *op_score = (BTScore *) packet->data;
    rep_.op_score_ = op_score->score_;
    if (keating_) {
      rep_.funds_ += keating_;
      keating_ = 0;
    } else if (tax_on_) {

      // I always hated accounting...
      if (op_score->funds_ > rep_.op_funds_) 
	      rep_.funds_ += (long) (((1 / (1 - BT_MONDALE_RATE)) * 
	        (op_score->funds_ - rep_.op_funds_)) * BT_MONDALE_RATE);
      tax_on_ -= (op_score->lines_ - rep_.op_lines_);
      if (tax_on_ < 0) 
	      tax_on_ = 0;
    }
    rep_.op_funds_ = op_score->funds_;
    if (rep_.op_funds_ > max_op_funds_)
      max_op_funds_ = rep_.op_funds_;

    rep_.op_lines_ = op_score->lines_;

    int new_lines_til_baz = BT_LINES_TIL_BAZ - 
      (rep_.op_lines_+ rep_.lines_) % BT_LINES_TIL_BAZ;

    if (new_lines_til_baz > lines_til_baz_) {
      send (BT_START_BAZ, 0);
    }
    lines_til_baz_ = new_lines_til_baz;

    if ( recon_ )
      spy_on_ = recon_->spy_on_;

    updateDisplay();

    break;
  }

  case BT_LINE: {
    BTLine *lines = (BTLine *) packet->data;
    rep_.lines_ += lines->inc();
    int new_lines_til_baz = BT_LINES_TIL_BAZ - 
      (rep_.op_lines_ + rep_.lines_) % BT_LINES_TIL_BAZ;
    if (new_lines_til_baz > lines_til_baz_) {
      send (BT_START_BAZ, 0);
    }
    lines_til_baz_ = new_lines_til_baz;
    break;
  }

  case BT_FUNDS: {
    if (!weapon_manager_->BTActive[BT_MONDALE]) 
      rep_.funds_ += *((short *) packet->data);    
    else
      rep_.funds_ += (long) ((1 - BT_MONDALE_RATE) * *((short *) packet->data));
    if (rep_.funds_ > max_funds_)
      max_funds_ = rep_.funds_;
    break;
  }
  }

  if (need_update_) {
  }    
  pass (packet);
}

void BTScoreManager::update() {
  updateDisplay();
  send (BT_SCORE, &rep_);
} 

BTScoreManager::~BTScoreManager() {
  if ( widget_rep_ )
    delete widget_rep_;
}
E 1
