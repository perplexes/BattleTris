h07994
s 00001/00001/00199
d D 1.2 01/10/21 19:25:07 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:26 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTChallengeDialog.C
c Name history : 1 0 src/game/BTChallengeDialog.C
e
s 00200/00000/00000
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
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTChallengeDialog.C                                 */
/*    ASSN:                                                     */
/*    DATE: Wed May  4 17:51:49 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <iostream.h>

#include "BTChallengeDialog.H"
#include "BTPlayer.H"

#define BT_CHAL_D_W 600
#define BT_CHAL_D_H 200
#define BT_CHAL_D_INFO_W 35

void BTChallengeDialog::_chal_cb_ (Widget, XtPointer thisp, XtPointer) {
//  BTChallengeDialog *dialog = (BTChallengeDialog *) thisp;
//  dialog->shell_->size( BT_CHAL_D_W, BT_CHAL_D_H );
//  dialog->size(400, 400);
}

BTChallengeDialog::BTChallengeDialog (BTWidget *parent,
				      char *headline, char *byline) 
: BTWidget (parent), chal_info_(0), shell_(0), chal_smack_(0), challenger_(0),
  accept_(0), decline_(0) {
  
  Visual *visual;
  Pixmap bg_pixmap;
  Pixmap brdr_pixmap;
  Colormap colormap;
  int depth;

  me_ = 0;

  XtVaGetValues (*parent_, 
    XmNvisual, &visual,
    XmNbackgroundPixmap, &bg_pixmap,
    XmNborderPixmap, &brdr_pixmap,
    XmNcolormap, &colormap,
    XmNdepth, &depth,
    0);

D 3
  me_ = XmCreateDialogShell (*parent_, "challenge_popup", 0, 0);
E 3
I 3
  me_ = XmCreateDialogShell (*parent_, (char *)"challenge_popup", 0, 0);
E 3
  size( BT_CHAL_D_W, BT_CHAL_D_H );
  noResize();

  XtVaSetValues(*this,
		XmNdialogType, XmDIALOG_WARNING,
		XmNdialogStyle, XmDIALOG_FULL_APPLICATION_MODAL,
		XmNvisual, visual,
		XmNbackgroundPixmap, bg_pixmap,
		XmNborderPixmap, brdr_pixmap,
		XmNcolormap, colormap,
		XmNdepth, depth,
		0);

  form_  = new BTFormWidget(this, "form",
			    BT_CHAL_D_W, BT_CHAL_D_H, 100 );

  chal_info_ = new BTScrolledTextWidget( form_, "chal_info", "Rookie" );
/*
  chal_info_->attachLeftForm();
  chal_info_->attachTopForm();
  chal_info_->attachBottomForm();
  chal_info_->attachRightPosition( 45 );
  */
  form_->placeChild( chal_info_, 1, 2, 44, 98 );
  chal_info_->manage();

  challenger_ = new BTLabelWidget(form_, "challenger", "mws@cs.brown.edu");
/*
  challenger_->attachTopForm();
  challenger_->topOffset( 10 );
  challenger_->attachLeftWidget( chal_info_ );
  challenger_->attachRightForm();
  challenger_->rightOffset( 10 );
  challenger_->attachBottomPosition( 30 );
  challenger_->leftOffset( 5 );
  */
  form_->placeChild( challenger_, 45, 10, 100, 30 );
  challenger_->manage();

  chal_smack_ = new BTLabelWidget(form_, "chal_smack", "wants a piece of your ass");
/*
  chal_smack_->attachTopWidget( challenger_ );
  chal_smack_->attachLeftWidget( chal_info_ );
  chal_smack_->attachRightForm();
  chal_smack_->rightOffset(10);
  chal_smack_->attachBottomPosition( 55 );
  chal_smack_->leftOffset( 5 );
*/
  form_->placeChild( chal_smack_, 45, 35, 100, 55 );
  chal_smack_->manage();

  accept_ = new BTPushButtonWidget( form_, "accept", "Accept" );
//  accept_->attachTopWidget( chal_smack_ );
  form_->placeChild( accept_, 47, 70, 67, 90 );
  accept_->manage();

  decline_ = new BTPushButtonWidget( form_, "decline", "Decline" );
  form_->placeChild( decline_, 77, 70, 97, 90 );
  decline_->manage();
  
/*
  BTFormWidget *form = new BTFormWidget (shell_, "form", 200, 200, 100);
 
  me_ = form->getWidget();

  if (headline) {
    XmString text = XmStringCreateSimple (headline);

    headline_ = new BTLabelWidget(form, "headline", headline);
    headline_->attachLeftForm();
    headline_->attachTopForm();
    headline_->topOffset(20);

    XmFontList fontlist;
    
    XtVaGetValues (*headline_, XmNfontList, &fontlist, 0); 
    unsigned short width = XmStringWidth (fontlist, text) + 80;
  
    XtVaSetValues (me_,
		   XmNwidth, width,
		   XmNdialogStyle, XmDIALOG_FULL_APPLICATION_MODAL,
		   0);

    unsigned short height = XmStringHeight (fontlist, text);
  
    if (byline) {
      XmString by_text = XmStringCreateSimple (byline);
      byline_ = new BTLabelWidget(form, "byline", byline);
      byline_->size(-1, -1, width);
      byline_->attachTopWidget( headline_ );
      byline_->attachLeftForm();

      XmStringFree (by_text);
    }

    XmStringFree (text);
    headline_->size( -1, -1, width );
    headline_->noResize();
  }

  yes_button_ 
    = new BTPushButtonWidget (form, "yes_button", " Bring 'em on. ");
  yes_button_->attachTopWidget( byline_ );
  yes_button_->topOffset(20);
  yes_button_->attachLeftForm();
  yes_button_->leftOffset(40);

  char q = 34;
  char msg[255];
  msg[0] = ' '; msg[1] = q; msg[2] = 0;
  strcat (msg, "Mommy!");
  strcat (msg, &q);  strcat (msg, " ");

  no_button_  
    = new BTPushButtonWidget (form, "no_button", msg);
  no_button_->attachTopWidget( byline_ );
  no_button_->topOffset(20);
  no_button_->attachRightForm();
  no_button_->rightOffset(40);
  */
  
  XtAddCallback (*form_, XmNmapCallback, _chal_cb_, this);
  width_ = 0;
}

void BTChallengeDialog::player(BTPlayer *player) {
  challenger_->setLabel(player->key());
  chal_info_->setText(player->formatInfo(), 1, BT_CHAL_D_INFO_W );
}

void BTChallengeDialog::show() {
  form_->manage();
}

void BTChallengeDialog::hide() {
  form_->unmanage();
//  BTWidget::unmanage();
}

BTChallengeDialog::~BTChallengeDialog() {
    if ( accept_ )
      delete accept_;
    if ( decline_ )
      delete decline_;
    if ( challenger_ )
      delete challenger_;
    if ( chal_smack_ )
      delete chal_smack_;
    if ( chal_info_ )
      delete chal_info_;
    if ( form_ )
      delete form_;
}
E 1
