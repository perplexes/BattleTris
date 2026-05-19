h41977
s 00000/00000/00000
d R 1.2 01/10/20 13:35:34 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTAbout.C
c Name history : 1 0 src/game/BTAbout.C
e
s 00239/00000/00000
d D 1.1 01/10/20 13:35:33 bmc 1 0
c date and time created 01/10/20 13:35:33 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTAbout.C                                           */
/*    ASGN: Final                                               */
/*    DATE: Mon May  9 20:04:33 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTConstants.H"
#include "BTAbout.H"
#include "BTPixmap.H"

const char *thanks_str[] = { 
"battletris@cs.brown.edu", "mail",
"", 0,
"BattleTris Copyright (c) 1993-1997 Bryan Cantrill, Charles Hoecker, Michael Shapiro.", 0,
"Special thanks to:", "thanks_title",
"Libby \"Hoss the Camel\" Cantrill, for many ideas and extensive play-testing", 0,
"Drew Davis, for great advice early on", 0,
"Tony, for cleaning up our empty Mountain Dew bottles", 0,
"botrytis, pebbles and barney for many long and passionate nights", 0, 
"The original BT beta testers:  Ben, Caffer, Masi, Dave, Scott and Todd", 0,
"and of course", "thanks_of_course",
"Kevin \"shouldn't there be a paren there?\" Regan", 0,
0
};

#define BT_ABOUT_FRAC_BASE 800

#define XFIX BT_ABOUT_FRAC_BASE / BT_ABOUT_WIDTH
#define YFIX BT_ABOUT_FRAC_BASE / BT_ABOUT_HEIGHT

#define BT_ABOUT_TITLE_Y1 20 * YFIX
#define BT_ABOUT_TITLE_Y2 100 * YFIX

#define BT_ABOUT_VERSION_Y1 115 * YFIX
#define BT_ABOUT_VERSION_Y2 130 * YFIX

#define BT_ABOUT_BRYAN_Y1 140 * YFIX
#define BT_ABOUT_BRYAN_Y2 170 * YFIX

#define BT_ABOUT_IMAGE_Y1 100 * YFIX
#define BT_ABOUT_IMAGE_Y2 250 * YFIX

#define BT_ABOUT_IMAGE_X1 50 * XFIX

#define BT_ABOUT_CHARLIE_Y1 170 * YFIX
#define BT_ABOUT_CHARLIE_Y2 210 * YFIX

#define BT_ABOUT_MIKE_Y1 210 * YFIX
#define BT_ABOUT_MIKE_Y2 240 * YFIX

#define BT_ABOUT_THANKS_Y1 245 * YFIX
#define BT_ABOUT_THANKS_Y2 275 * YFIX

#define BT_ABOUT_SPECIAL_Y1 450 * YFIX
#define BT_ABOUT_SPECIAL_Y2 480 * YFIX

#define BT_ABOUT_LOGO_LEFT_X1 50
#define BT_ABOUT_LOGO_LEFT_Y1 150

#define BT_ABOUT_OK_BUTTON_X1 260 * XFIX
#define BT_ABOUT_OK_BUTTON_Y1 535 * YFIX
#define BT_ABOUT_OK_BUTTON_X2 380 * XFIX
#define BT_ABOUT_OK_BUTTON_Y2 585 * YFIX

#define BT_ABOUT_OFFSET 30

BTAbout::BTAbout(BTWidget *parent, BTPixmap *image)
{
  static char labelbuf[64];
  click_ = 0;
  int i;

  form_ = new BTFormWidget(parent, "BTAbout", BT_ABOUT_WIDTH,
			   BT_ABOUT_HEIGHT, BT_ABOUT_FRAC_BASE);

  for(i = 0; i < BT_MAX_EGGS; i++)
    eggs_[i] = 0;

  title_ = new BTLabelWidget( form_, "title", "BattleTris", 640, 50 );

  title_->attachLeftForm();
  title_->attachRightForm();
  title_->attachTopPosition( BT_ABOUT_TITLE_Y1 );
  title_->attachBottomPosition( BT_ABOUT_TITLE_Y2 );

  title_->manage();

  left_drawing_area_ =
    new BTDrawingAreaWidget(form_, "drawing_area", image,
                            image->width_, image->height_);

  left_drawing_area_->attachLeftForm();
  left_drawing_area_->attachTopForm();
  left_drawing_area_->leftOffset( BT_ABOUT_IMAGE_X1 );
  left_drawing_area_->topOffset( BT_ABOUT_IMAGE_Y1 );

  left_drawing_area_->manage();

  right_drawing_area_ =
    new BTDrawingAreaWidget(form_, "drawing_area", image,
                            image->width_, image->height_);

  right_drawing_area_->attachRightForm();
  right_drawing_area_->attachTopForm();
  right_drawing_area_->rightOffset( BT_ABOUT_IMAGE_X1 );
  right_drawing_area_->topOffset( BT_ABOUT_IMAGE_Y1 );

  right_drawing_area_->manage();

  sprintf(labelbuf, "Version %d.%d", BT_MAJOR_VER, BT_MINOR_VER);
  version_ = new BTLabelWidget(form_, "version", labelbuf, 640, 50);

  version_->attachLeftWidget(left_drawing_area_);
  version_->attachRightWidget(right_drawing_area_);
  version_->attachTopPosition( BT_ABOUT_VERSION_Y1 );
  version_->attachBottomPosition( BT_ABOUT_VERSION_Y2 );

  version_->manage();

  left_drawing_area_->addInputCallback( handleLeftClick, this );
  right_drawing_area_->addInputCallback( handleRightClick, this );

  bryan_ = new BTLabelWidget(form_, "bryan", "Bryan Cantrill");

  bryan_->attachLeftWidget( left_drawing_area_ );
  bryan_->attachRightWidget( right_drawing_area_ );
  bryan_->attachTopPosition( BT_ABOUT_BRYAN_Y1 );
  bryan_->attachBottomPosition( BT_ABOUT_BRYAN_Y2 );

  bryan_->addInputCallback( handleBryanClick, this );

  bryan_->manage();

  charlie_ = new BTLabelWidget(form_, "charlie", "Charlie Hoecker", 640, 50);

  charlie_->attachLeftForm();
  charlie_->attachRightForm();
  charlie_->attachTopPosition( BT_ABOUT_CHARLIE_Y1 );
  charlie_->attachBottomPosition( BT_ABOUT_CHARLIE_Y2 );

  charlie_->addInputCallback( handleCharlieClick, this );

  charlie_->manage();

  mike_ = new BTLabelWidget(form_, "mike", "Mike Shapiro", 640, 50);

  mike_->attachLeftForm();
  mike_->attachRightForm();
  mike_->attachTopPosition( BT_ABOUT_MIKE_Y1 );
  mike_->attachBottomPosition( BT_ABOUT_MIKE_Y2 );

  mike_->addInputCallback( handleMikeClick, this );

  mike_->manage();

  ok_button_ =
    new BTPushButtonWidget(form_, "ok_button", "OK");
  form_->placeChild( ok_button_, BT_ABOUT_OK_BUTTON_X1,
		     BT_ABOUT_OK_BUTTON_Y1,
		     BT_ABOUT_OK_BUTTON_X2,
		     BT_ABOUT_OK_BUTTON_Y2 );

  ok_button_->manage();

  for (i = 0; thanks_str[i] != 0; i++) {

    char name[255];

    thanks_[i / 2] = new BTLabelWidget( form_, 
      thanks_str[i + 1] ? (char *) thanks_str[i + 1] : "thanks",
      (char *) thanks_str[i], 640, 50);

    if(strstr(thanks_str[i], "Libby")) {
      thanks_[i/2]->addInputCallback( handleLibbyClick, this );
    }

    if(strstr(thanks_str[i], "Kevin")) {
      thanks_[i/2]->addInputCallback( handleKevinClick, this );
    }

    thanks_[i/2]->attachLeftForm();
    thanks_[i/2]->attachRightForm();
    thanks_[i/2]->attachTopPosition(BT_ABOUT_THANKS_Y1 + (i / 2) * BT_ABOUT_OFFSET);
    thanks_[i/2]->attachBottomPosition(BT_ABOUT_THANKS_Y2 + (i / 2) * BT_ABOUT_OFFSET);

    thanks_[i / 2]->manage();
    i++;
  }
}

void BTAbout::handleLeftClick (BTWidget *, void *thisp) {
  ((BTAbout *) thisp)->click_ = 1;
}

void BTAbout::handleRightClick (BTWidget *, void *thisp) {
  if (((BTAbout *) thisp)->click_ >= 1)
    ((BTAbout *) thisp)->click_ = 2;
  else
    ((BTAbout *) thisp)->click_ = 0;
}

void BTAbout::handleBryanClick (BTWidget *, void *thisp) {
  ((BTAbout *) thisp)->eggs_[BT_BRYAN_EGG] = 1;
}

void BTAbout::handleCharlieClick (BTWidget *, void *thisp) {
  ((BTAbout *) thisp)->eggs_[BT_CHARLIE_EGG] = 1;
}

void BTAbout::handleMikeClick (BTWidget *, void *thisp) {
  ((BTAbout *) thisp)->eggs_[BT_MIKE_EGG] = 1;
}

void BTAbout::handleLibbyClick (BTWidget *, void *thisp) {
  ((BTAbout *) thisp)->eggs_[BT_LIBBY_EGG] = 1;
}

void BTAbout::handleKevinClick (BTWidget *, void *thisp) {
  ((BTAbout *) thisp)->eggs_[BT_KEVIN_EGG] = 1;
}

BTAbout::~BTAbout()
{
  for (int i = 0; thanks_str[i] != 0; i+=2) {
    delete thanks_[i/2];
  }
  delete ok_button_;
  delete mike_;
  delete charlie_;
  delete bryan_;
  delete right_drawing_area_;
  delete left_drawing_area_;
  delete version_;
  delete title_;
  delete form_;
}
E 1
