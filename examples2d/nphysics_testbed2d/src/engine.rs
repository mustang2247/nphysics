use std::rc::Rc;
use std::cell::RefCell;
use std::sync::Arc;
use std::collections::HashMap;
use rand::{SeedableRng, XorShiftRng, Rng};
use sfml::graphics::RenderWindow;
use na::{Pnt2, Pnt3, Iso2};
use na;
use nphysics2d::object::{WorldObject, RigidBodyHandle, SensorHandle};
use ncollide::inspection::Repr2;
use ncollide::shape;
use camera::Camera;
use objects::{SceneNode, Ball, Box, Lines, Segment};

pub type GraphicsManagerHandle = Rc<RefCell<GraphicsManager<'static>>>;

pub struct GraphicsManager<'a> {
    // NOTE: sensors and rigid bodies are not on the same hashmap because we want do draw sensors
    // after all the rigid bodies.
    rand:      XorShiftRng,
    rb2sn:     HashMap<usize, Vec<SceneNode<'a>>>,
    s2sn:      HashMap<usize, Vec<SceneNode<'a>>>,
    obj2color: HashMap<usize, Pnt3<u8>>
}

impl<'a> GraphicsManager<'a> {
    pub fn new() -> GraphicsManager<'a> {
        GraphicsManager {
            rand:      SeedableRng::from_seed([0, 1, 2, 3]),
            rb2sn:     HashMap::new(),
            s2sn:      HashMap::new(),
            obj2color: HashMap::new()
        }
    }

    pub fn add(&mut self, object: WorldObject<f32>) {
        let nodes = {
            let bobject = object.borrow();
            let mut nodes = Vec::new();

            self.add_shape(object.clone(), na::one(), bobject.shape().as_ref(), &mut nodes);

            nodes
        };

        match object {
            WorldObject::RigidBody(ref rb) => { self.rb2sn.insert(WorldObject::rigid_body_uid(rb), nodes); },
            WorldObject::Sensor(ref s)     => { self.s2sn.insert(WorldObject::sensor_uid(s), nodes); }
        }
    }

    fn add_shape(&mut self,
                 object: WorldObject<f32>,
                 delta:  Iso2<f32>,
                 shape:  &Repr2<f32>,
                 out:    &mut Vec<SceneNode<'a>>) {
        type Pl = shape::Plane2<f32>;
        type Bl = shape::Ball2<f32>;
        type Cx = shape::Convex2<f32>;
        type Bo = shape::Cuboid2<f32>;
        type Cy = shape::Cylinder2<f32>;
        type Co = shape::Cone2<f32>;
        type Cm = shape::Compound2<f32>;
        type Ls = shape::Polyline2<f32>;
        type Se = shape::Segment2<f32>;

        let repr = shape.repr();

        if let Some(s) = repr.downcast_ref::<Pl>() {
            self.add_plane(object, s, out)
        }
        else if let Some(s) = repr.downcast_ref::<Bl>() {
            self.add_ball(object, delta, s, out)
        }
        else if let Some(s) = repr.downcast_ref::<Bo>() {
            self.add_box(object, delta, s, out)
        }
        else if let Some(s) = repr.downcast_ref::<Cx>() {
            self.add_convex(object, delta, s, out)
        }
        else if let Some(s) = repr.downcast_ref::<Se>() {
            self.add_segment(object, delta, s, out)
        }
        else if let Some(s) = repr.downcast_ref::<Cm>() {
            for &(t, ref s) in s.shapes().iter() {
                self.add_shape(object.clone(), delta * t, s.as_ref(), out)
            }
        }
        else if let Some(s) = repr.downcast_ref::<Ls>() {
            self.add_lines(object, delta, s, out)
        }
        else {
            panic!("Not yet implemented.")
        }

    }

    fn add_plane(&mut self,
                 _: WorldObject<f32>,
                 _: &shape::Plane2<f32>,
                 _: &mut Vec<SceneNode>) {
    }

    fn add_ball(&mut self,
                object: WorldObject<f32>,
                delta:  Iso2<f32>,
                shape:  &shape::Ball2<f32>,
                out:    &mut Vec<SceneNode>) {
        let color = self.color_for_object(&object);
        let margin = object.borrow().margin();
        out.push(SceneNode::BallNode(Ball::new(object, delta, shape.radius() + margin, color)))
    }
    
    fn add_convex(&mut self,
                  object: WorldObject<f32>,
                  delta:  Iso2<f32>,
                  shape:  &shape::Convex2<f32>,
                  out:    &mut Vec<SceneNode>) {
        let color = self.color_for_object(&object);
        //let margin = object.borrow().margin();
        let points = shape.points();
        let vector = points.iter().cloned().collect();
        let vs = Arc::new(vector);
        let is = {
	    let limit = shape.points().len();
	    Arc::new( (0..limit as usize).map(|x| Pnt2::new(x, (x+(1 as usize)) % limit )).collect() )
        };
        
        out.push(SceneNode::LinesNode(Lines::new(object, delta, vs, is, color)))
    }

    fn add_lines(&mut self,
                 object: WorldObject<f32>,
                 delta:  Iso2<f32>,
                 shape:  &shape::Polyline2<f32>,
                 out:    &mut Vec<SceneNode>) {

        let color = self.color_for_object(&object);

        let vs = shape.vertices().clone();
        let is = shape.indices().clone();

        out.push(SceneNode::LinesNode(Lines::new(object, delta, vs, is, color)))
    }


    fn add_box(&mut self,
               object: WorldObject<f32>,
               delta:  Iso2<f32>,
               shape:  &shape::Cuboid2<f32>,
               out:    &mut Vec<SceneNode>) {
        let rx = shape.half_extents().x;
        let ry = shape.half_extents().y;
        let margin = object.borrow().margin();

        let color = self.color_for_object(&object);

        out.push(SceneNode::BoxNode(Box::new(object, delta, rx + margin, ry + margin, color)))
    }

    fn add_segment(&mut self,
                   object: WorldObject<f32>,
                   delta:  Iso2<f32>,
                   shape:  &shape::Segment2<f32>,
                   out:    &mut Vec<SceneNode>) {
        let a = shape.a();
        let b = shape.b();

        let color = self.color_for_object(&object);

        out.push(SceneNode::SegmentNode(Segment::new(object, delta, *a, *b, color)))
    }


    pub fn clear(&mut self) {
        self.rb2sn.clear();
        self.s2sn.clear();
    }

    pub fn draw(&mut self, rw: &mut RenderWindow, c: &Camera) {
        c.activate_scene(rw);

        for (_, ns) in self.rb2sn.iter_mut().chain(self.s2sn.iter_mut()) {
            for n in ns.iter_mut() {
                match *n {
                    SceneNode::BoxNode(ref mut n)     => n.update(),
                    SceneNode::BallNode(ref mut n)    => n.update(),
                    SceneNode::LinesNode(ref mut n)   => n.update(),
                    SceneNode::SegmentNode(ref mut n) => n.update(),
                }
            }
        }

        for (_, ns) in self.rb2sn.iter_mut().chain(self.s2sn.iter_mut()) {
            for n in ns.iter_mut() {
                match *n {
                    SceneNode::BoxNode(ref n)     => n.draw(rw),
                    SceneNode::BallNode(ref n)    => n.draw(rw),
                    SceneNode::LinesNode(ref n)   => n.draw(rw),
                    SceneNode::SegmentNode(ref n) => n.draw(rw),
                }
            }
        }

        c.activate_ui(rw);
    }

    fn set_color(&mut self, key: usize, color: Pnt3<f32>) {
        let color = Pnt3::new(
            (color.x * 255.0) as u8,
            (color.y * 255.0) as u8,
            (color.z * 255.0) as u8
        );

        self.obj2color.insert(key, color);

        if let Some(sns) = self.rb2sn.get_mut(&key) {
            for sn in sns.iter_mut() {
                sn.set_color(color)
            }
        }

        if let Some(sns) = self.s2sn.get_mut(&key) {
            for sn in sns.iter_mut() {
                sn.set_color(color)
            }
        }
    }

    pub fn set_rigid_body_color(&mut self, object: &RigidBodyHandle<f32>, color: Pnt3<f32>) {
        self.set_color(WorldObject::rigid_body_uid(object), color)
    }

    pub fn set_sensor_color(&mut self, sensor: &SensorHandle<f32>, color: Pnt3<f32>) {
        self.set_color(WorldObject::sensor_uid(sensor), color)
    }

    pub fn color_for_object(&mut self, object: &WorldObject<f32>) -> Pnt3<u8> {
        let key = object.uid();
        match self.obj2color.get(&key) {
            Some(color) => return *color,
            None        => { }
        }

        let mut color = Pnt3::new(
            self.rand.gen_range(0usize, 256) as u8,
            self.rand.gen_range(0usize, 256) as u8,
            self.rand.gen_range(0usize, 256) as u8);

        if let WorldObject::Sensor(ref s) = *object {
            if let Some(parent) = s.borrow().parent() {
                if let Some(pcolor) = self.obj2color.get(&WorldObject::rigid_body_uid(parent)) {
                    color = *pcolor;
                }
            }
        }


        self.obj2color.insert(key, color);

        color
    }

    pub fn rigid_body_to_scene_node(&mut self, rb: &RigidBodyHandle<f32>) -> Option<&mut Vec<SceneNode<'a>>> {
        self.rb2sn.get_mut(&WorldObject::rigid_body_uid(rb))
    }

    pub fn sensor_to_scene_node(&mut self, sensor: &SensorHandle<f32>) -> Option<&mut Vec<SceneNode<'a>>> {
        self.s2sn.get_mut(&WorldObject::sensor_uid(sensor))
    }
}
