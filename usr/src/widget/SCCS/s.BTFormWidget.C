h36167
s 00030/00031/00019
d D 1.2 01/10/21 19:25:14 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:13 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTFormWidget.C
c Name history : 1 0 src/widget/BTFormWidget.C
e
s 00050/00000/00000
d D 1.1 01/10/20 13:35:12 bmc 1 0
c date and time created 01/10/20 13:35:12 by bmc
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
/*    FILE: BTFormWidget.C                                      */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 18:02:23 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTFormWidget.H"
#include "BattleTris.H"

D 3
BTFormWidget::BTFormWidget(BTWidget *parent, char *const name,
			   Dimension width, Dimension height, int fraction_base)
E 3
I 3
BTFormWidget::BTFormWidget(BTWidget *parent, const char *name,
    Dimension width, Dimension height, int fraction_base)
E 3
: BTWidget(parent), frac_base_(fraction_base)
{
D 3
  me_ = XtVaCreateWidget(name, xmFormWidgetClass, parent ? parent->getWidget() : 0,
			 XmNheight, height, XmNwidth, width,
			 XmNfractionBase, fraction_base,
			 XmNdepth, g_depth,
			 NULL);
E 3
I 3
	me_ = XtVaCreateWidget(name, xmFormWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNheight, height, XmNwidth, width,
	    XmNfractionBase, fraction_base,
	    XmNdepth, g_depth,
	    NULL);
E 3
}

D 3
void BTFormWidget::placeChild( BTWidget *child, Dimension x, Dimension y ) {

  XtVaSetValues( *child,
		 XmNleftAttachment, XmATTACH_POSITION,
		 XmNleftPosition, x,
		 XmNtopAttachment, XmATTACH_POSITION,
		 XmNtopPosition, y, 0 );

E 3
I 3
void
BTFormWidget::placeChild(BTWidget *child, Dimension x, Dimension y)
{
	XtVaSetValues(*child,
	    XmNleftAttachment, XmATTACH_POSITION,
	    XmNleftPosition, x,
	    XmNtopAttachment, XmATTACH_POSITION,
	    XmNtopPosition, y, 0);
E 3
}

D 3
void BTFormWidget::placeChild( BTWidget *child, Dimension x1, Dimension y1,
			       Dimension x2, Dimension y2 ) {

  XtVaSetValues( *child,
		 XmNleftAttachment, XmATTACH_POSITION,
		 XmNleftPosition, x1,
		 XmNtopAttachment, XmATTACH_POSITION,
		 XmNtopPosition, y1,
		 XmNrightAttachment, XmATTACH_POSITION,
		 XmNrightPosition, x2,
		 XmNbottomAttachment, XmATTACH_POSITION,
		 XmNbottomPosition, y2,
//		 XmNwidth, ((x2-x1) * width()) / frac_base_,
//		 XmNheight, ((y2-y1) * height()) / frac_base_,
		 0 );
E 3
I 3
void
BTFormWidget::placeChild(BTWidget *child, Dimension x1, Dimension y1,
    Dimension x2, Dimension y2)
{
	XtVaSetValues(*child,
	    XmNleftAttachment, XmATTACH_POSITION,
	    XmNleftPosition, x1,
	    XmNtopAttachment, XmATTACH_POSITION,
	    XmNtopPosition, y1,
	    XmNrightAttachment, XmATTACH_POSITION,
	    XmNrightPosition, x2,
	    XmNbottomAttachment, XmATTACH_POSITION,
	    XmNbottomPosition, y2,
	    0);
E 3
}
D 3

E 3
E 1
